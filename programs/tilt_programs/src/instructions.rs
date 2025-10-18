use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use crate::state::*;
use crate::errors::*;
use crate::poker;

/// Initialize a player account
pub fn initialize_player(ctx: Context<InitializePlayer>) -> Result<()> {
    let player = &mut ctx.accounts.player_account;
    player.authority = ctx.accounts.authority.key();
    player.total_hands_played = 0;
    player.total_hands_won = 0;
    player.total_winnings = 0;
    player.bump = *ctx.bumps.get("player_account").unwrap();
    
    Ok(())
}

#[derive(Accounts)]
pub struct InitializePlayer<'info> {
    #[account(
        init,
        payer = authority,
        space = PlayerAccount::LEN,
        seeds = [b"player", authority.key().as_ref()],
        bump
    )]
    pub player_account: Account<'info, PlayerAccount>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Create a new game
pub fn create_game(
    ctx: Context<CreateGame>,
    stake_amount: u64,
    commitment: [u8; 32],
) -> Result<()> {
    require!(commitment != [0u8; 32], PokerError::ZeroCommitment);
    
    let game = &mut ctx.accounts.game_state;
    let clock = Clock::get()?;
    
    // Initialize game state
    game.game_id = clock.unix_timestamp as u64;
    game.player1 = ctx.accounts.player1.key();
    game.player2 = Pubkey::default();
    game.stake_amount = stake_amount;
    game.pot = 0;
    game.player1_current_bet = 0;
    game.player2_current_bet = 0;
    
    // Commitments
    game.player1_commitment = commitment;
    game.player2_commitment = [0u8; 32];
    game.player1_secret_revealed = false;
    game.player2_secret_revealed = false;
    game.player1_secret = [0u8; 32];
    game.player2_secret = [0u8; 32];
    
    // Initialize deck and hands
    game.deck = [0u8; 52];
    game.next_card_index = 0;
    game.player1_hand = [0u8; 2];
    game.player2_hand = [0u8; 2];
    game.community_cards = [0u8; 5];
    game.community_cards_dealt = 0;
    
    // Game state
    game.stage = GameStage::WaitingForPlayers;
    game.current_player = 0;
    game.dealer_button = 1; // Player 1 is dealer
    game.last_action = PlayerActionType::None;
    
    // Blinds (configurable, but standard is SB=1, BB=2 in chips)
    game.small_blind = stake_amount / 100; // 1% of stake
    game.big_blind = stake_amount / 50; // 2% of stake
    
    // Player states
    game.player1_folded = false;
    game.player2_folded = false;
    game.player1_all_in = false;
    game.player2_all_in = false;
    
    // Timing
    game.created_at = clock.unix_timestamp;
    game.last_action_at = clock.unix_timestamp;
    game.action_timeout = 60; // 60 seconds per action
    
    // Result
    game.winner = None;
    game.winning_hand_rank = None;
    
    game.bump = *ctx.bumps.get("game_state").unwrap();
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(stake_amount: u64)]
pub struct CreateGame<'info> {
    #[account(
        init,
        payer = player1,
        space = GameState::LEN,
        seeds = [
            b"game",
            player1.key().as_ref(),
            &Clock::get()?.unix_timestamp.to_le_bytes()
        ],
        bump
    )]
    pub game_state: Account<'info, GameState>,
    
    #[account(mut)]
    pub player1: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

/// Player 2 joins the game
pub fn join_game(
    ctx: Context<JoinGame>,
    commitment: [u8; 32],
) -> Result<()> {
    require!(commitment != [0u8; 32], PokerError::ZeroCommitment);
    
    let game = &mut ctx.accounts.game_state;
    let clock = Clock::get()?;
    
    require!(
        game.stage == GameStage::WaitingForPlayers,
        PokerError::InvalidGameStage
    );
    
    require!(
        game.player2 == Pubkey::default(),
        PokerError::GameAlreadyFull
    );
    
    require!(
        ctx.accounts.player2.key() != game.player1,
        PokerError::CannotJoinOwnGame
    );
    
    game.player2 = ctx.accounts.player2.key();
    game.player2_commitment = commitment;
    game.stage = GameStage::WaitingForReveals;
    game.last_action_at = clock.unix_timestamp;
    
    Ok(())
}

#[derive(Accounts)]
pub struct JoinGame<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
    #[account(mut)]
    pub player2: Signer<'info>,
}

/// Reveal secret for randomness generation
pub fn reveal_secret(
    ctx: Context<RevealSecret>,
    secret: [u8; 32],
) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let player = ctx.accounts.player.key();
    
    require!(
        game.stage == GameStage::WaitingForReveals,
        PokerError::InvalidGameStage
    );
    
    // Verify commitment
    let commitment_hash = keccak::hash(&secret).to_bytes();
    
    if player == game.player1 {
        require!(
            commitment_hash == game.player1_commitment,
            PokerError::SecretMismatch
        );
        game.player1_secret = secret;
        game.player1_secret_revealed = true;
    } else if player == game.player2 {
        require!(
            commitment_hash == game.player2_commitment,
            PokerError::SecretMismatch
        );
        game.player2_secret = secret;
        game.player2_secret_revealed = true;
    } else {
        return Err(PokerError::NotYourTurn.into());
    }
    
    // If both secrets revealed, initialize the deck and post blinds
    if game.player1_secret_revealed && game.player2_secret_revealed {
        // Combine secrets using XOR then hash
        let mut combined = [0u8; 32];
        for i in 0..32 {
            combined[i] = game.player1_secret[i] ^ game.player2_secret[i];
        }
        let combined_seed = keccak::hash(&combined).to_bytes();
        
        // Initialize shuffled deck
        game.initialize_deck(combined_seed);
        
        // Post blinds (in heads-up, button is SB and acts first pre-flop)
        if game.dealer_button == 1 {
            game.player1_current_bet = game.small_blind;
            game.player2_current_bet = game.big_blind;
            game.pot = game.small_blind + game.big_blind;
            game.current_player = 1; // SB acts first pre-flop
        } else {
            game.player2_current_bet = game.small_blind;
            game.player1_current_bet = game.big_blind;
            game.pot = game.small_blind + game.big_blind;
            game.current_player = 2;
        }
        
        game.stage = GameStage::PreFlop;
    }
    
    let clock = Clock::get()?;
    game.last_action_at = clock.unix_timestamp;
    
    Ok(())
}

#[derive(Accounts)]
pub struct RevealSecret<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
    pub player: Signer<'info>,
}

/// Deal initial cards (pocket cards)
pub fn deal_initial(ctx: Context<DealInitial>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    require!(
        game.stage == GameStage::PreFlop,
        PokerError::InvalidGameStage
    );
    
    require!(
        game.player1_hand == [0u8; 2],
        PokerError::CardsAlreadyDealt
    );
    
    // Deal 2 cards to each player
    game.player1_hand[0] = game.deal_card();
    game.player2_hand[0] = game.deal_card();
    game.player1_hand[1] = game.deal_card();
    game.player2_hand[1] = game.deal_card();
    
    Ok(())
}

#[derive(Accounts)]
pub struct DealInitial<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
}

/// Handle player actions (fold, check, call, raise)
pub fn player_action(
    ctx: Context<PlayerAction>,
    action: PlayerActionType,
    raise_amount: Option<u64>,
) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let player = ctx.accounts.player.key();
    let clock = Clock::get()?;
    
    // Verify it's a valid betting stage
    require!(
        matches!(
            game.stage,
            GameStage::PreFlop | GameStage::Flop | GameStage::Turn | GameStage::River
        ),
        PokerError::InvalidGameStage
    );
    
    // Verify it's the player's turn
    require!(game.is_player_turn(&player), PokerError::NotYourTurn);
    
    let is_player1 = player == game.player1;
    let current_bet = if is_player1 {
        game.player1_current_bet
    } else {
        game.player2_current_bet
    };
    
    let opponent_bet = if is_player1 {
        game.player2_current_bet
    } else {
        game.player1_current_bet
    };
    
    // Check if player has folded or is all-in
    if is_player1 {
        require!(!game.player1_folded, PokerError::CannotActAfterFold);
        require!(!game.player1_all_in, PokerError::CannotRaiseAllIn);
    } else {
        require!(!game.player2_folded, PokerError::CannotActAfterFold);
        require!(!game.player2_all_in, PokerError::CannotRaiseAllIn);
    }
    
    match action {
        PlayerActionType::Fold => {
            if is_player1 {
                game.player1_folded = true;
            } else {
                game.player2_folded = true;
            }
            game.stage = GameStage::Completed;
            game.winner = Some(game.get_other_player(&player));
        }
        
        PlayerActionType::Check => {
            // Can only check if bets are equal
            require!(
                current_bet == opponent_bet,
                PokerError::InvalidAction
            );
        }
        
        PlayerActionType::Call => {
            let call_amount = opponent_bet.saturating_sub(current_bet);
            
            if is_player1 {
                game.player1_current_bet = opponent_bet;
            } else {
                game.player2_current_bet = opponent_bet;
            }
            
            game.pot = game.pot.saturating_add(call_amount);
        }
        
        PlayerActionType::Raise => {
            let raise_amt = raise_amount.ok_or(PokerError::InvalidBetAmount)?;
            let min_raise = opponent_bet.saturating_sub(current_bet) * 2;
            
            require!(
                raise_amt >= min_raise && raise_amt >= opponent_bet,
                PokerError::MinimumRaiseNotMet
            );
            
            let total_bet = current_bet.saturating_add(raise_amt);
            
            if is_player1 {
                game.player1_current_bet = total_bet;
            } else {
                game.player2_current_bet = total_bet;
            }
            
            game.pot = game.pot.saturating_add(raise_amt);
        }
        
        PlayerActionType::AllIn => {
            // Player goes all-in with remaining chips
            if is_player1 {
                game.player1_all_in = true;
            } else {
                game.player2_all_in = true;
            }
        }
        
        PlayerActionType::None => {
            return Err(PokerError::InvalidAction.into());
        }
    }
    
    game.last_action = action;
    game.last_action_at = clock.unix_timestamp;
    
    // Switch current player
    game.current_player = if game.current_player == 1 { 2 } else { 1 };
    
    Ok(())
}

#[derive(Accounts)]
pub struct PlayerAction<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
    pub player: Signer<'info>,
}

/// Advance to next street (flop, turn, river) or showdown
pub fn advance_street(ctx: Context<AdvanceStreet>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    // Check if betting round is complete
    require!(
        game.is_betting_round_complete(),
        PokerError::BettingRoundNotComplete
    );
    
    // If someone folded, game is over
    if game.player1_folded || game.player2_folded {
        game.stage = GameStage::Completed;
        return Ok(());
    }
    
    // Reset current bets for new street
    game.player1_current_bet = 0;
    game.player2_current_bet = 0;
    
    // In heads-up, big blind acts first post-flop
    game.current_player = if game.dealer_button == 1 { 2 } else { 1 };
    
    match game.stage {
        GameStage::PreFlop => {
            // Deal flop (3 cards)
            game.community_cards[0] = game.deal_card();
            game.community_cards[1] = game.deal_card();
            game.community_cards[2] = game.deal_card();
            game.community_cards_dealt = 3;
            game.stage = GameStage::Flop;
        }
        
        GameStage::Flop => {
            // Deal turn (1 card)
            game.community_cards[3] = game.deal_card();
            game.community_cards_dealt = 4;
            game.stage = GameStage::Turn;
        }
        
        GameStage::Turn => {
            // Deal river (1 card)
            game.community_cards[4] = game.deal_card();
            game.community_cards_dealt = 5;
            game.stage = GameStage::River;
        }
        
        GameStage::River => {
            // Go to showdown
            game.stage = GameStage::Showdown;
        }
        
        _ => {
            return Err(PokerError::InvalidGameStage.into());
        }
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct AdvanceStreet<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
}

/// Resolve the game and determine winner
pub fn resolve_game(ctx: Context<ResolveGame>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    require!(
        game.stage == GameStage::Showdown,
        PokerError::InvalidGameStage
    );
    
    // Evaluate both hands
    let (_, player1_score) = poker::find_best_hand(
        &game.player1_hand,
        &game.community_cards,
    );
    
    let (_, player2_score) = poker::find_best_hand(
        &game.player2_hand,
        &game.community_cards,
    );
    
    // Determine winner
    if player1_score > player2_score {
        game.winner = Some(game.player1);
        game.winning_hand_rank = Some((player1_score >> 20) as u16);
    } else if player2_score > player1_score {
        game.winner = Some(game.player2);
        game.winning_hand_rank = Some((player2_score >> 20) as u16);
    } else {
        // Split pot (tie)
        game.winner = None;
    }
    
    game.stage = GameStage::Completed;
    
    // Update player stats
    let player1_account = &mut ctx.accounts.player1_account;
    let player2_account = &mut ctx.accounts.player2_account;
    
    player1_account.total_hands_played += 1;
    player2_account.total_hands_played += 1;
    
    if let Some(winner) = game.winner {
        if winner == game.player1 {
            player1_account.total_hands_won += 1;
            player1_account.total_winnings = player1_account
                .total_winnings
                .saturating_add(game.pot as i64);
            player2_account.total_winnings = player2_account
                .total_winnings
                .saturating_sub(game.stake_amount as i64);
        } else {
            player2_account.total_hands_won += 1;
            player2_account.total_winnings = player2_account
                .total_winnings
                .saturating_add(game.pot as i64);
            player1_account.total_winnings = player1_account
                .total_winnings
                .saturating_sub(game.stake_amount as i64);
        }
    } else {
        // Split pot - both get their stake back
        let split = game.pot / 2;
        player1_account.total_winnings = player1_account
            .total_winnings
            .saturating_add(split as i64)
            .saturating_sub(game.stake_amount as i64);
        player2_account.total_winnings = player2_account
            .total_winnings
            .saturating_add(split as i64)
            .saturating_sub(game.stake_amount as i64);
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct ResolveGame<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
    #[account(
        mut,
        seeds = [b"player", game_state.player1.as_ref()],
        bump = player1_account.bump
    )]
    pub player1_account: Account<'info, PlayerAccount>,
    
    #[account(
        mut,
        seeds = [b"player", game_state.player2.as_ref()],
        bump = player2_account.bump
    )]
    pub player2_account: Account<'info, PlayerAccount>,
}

/// Claim timeout win if opponent doesn't act
pub fn claim_timeout(ctx: Context<ClaimTimeout>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let clock = Clock::get()?;
    let player = ctx.accounts.player.key();
    
    // Check that timeout has been reached
    let elapsed = clock.unix_timestamp - game.last_action_at;
    require!(
        elapsed > game.action_timeout,
        PokerError::TimeoutNotReached
    );
    
    // Verify it's not the claiming player's turn
    require!(!game.is_player_turn(&player), PokerError::NotYourTurn);
    
    // Award win to the player who didn't timeout
    game.winner = Some(player);
    game.stage = GameStage::Completed;
    
    Ok(())
}

#[derive(Accounts)]
pub struct ClaimTimeout<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
    pub player: Signer<'info>,
}

