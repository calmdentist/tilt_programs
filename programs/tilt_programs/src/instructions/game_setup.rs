use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use crate::state::*;
use crate::errors::*;

/// Create a new game
pub fn create_game(
    ctx: Context<CreateGame>,
    stake_amount: u64,
    player1_ephemeral_pubkey: EphemeralPubkey,
    game_id: u64,
) -> Result<()> {
    require!(stake_amount > 0, PokerError::InvalidBetAmount);
    require!(
        player1_ephemeral_pubkey.data != [0u8; 32],
        PokerError::InvalidEphemeralKey
    );
    
    // Bond amount (10% of stake)
    let bond_amount = stake_amount / 10;
    let total_amount = stake_amount + bond_amount;
    
    // Check player has sufficient balance
    let player_balance = &mut ctx.accounts.player1_balance;
    require!(
        player_balance.balance >= total_amount,
        PokerError::InsufficientBalanceToJoin
    );
    
    // Deduct stake + bond from player balance and transfer to game vault
    player_balance.balance = player_balance.balance.checked_sub(total_amount)
        .ok_or(PokerError::InsufficientBalanceToJoin)?;
    
    // Transfer USDC from program vault to game vault
    let seeds = &[
        b"program_vault".as_ref(),
        &[*ctx.bumps.get("program_vault_authority").unwrap()],
    ];
    let signer = &[&seeds[..]];
    
    let cpi_accounts = Transfer {
        from: ctx.accounts.program_vault.to_account_info(),
        to: ctx.accounts.game_vault.to_account_info(),
        authority: ctx.accounts.program_vault_authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, total_amount)?;
    
    let game = &mut ctx.accounts.game_state;
    let clock = Clock::get()?;
    
    // Initialize game state
    game.game_id = game_id;
    game.player1 = ctx.accounts.player1.key();
    game.player2 = Pubkey::default();
    game.token_vault = ctx.accounts.game_vault.key();
    game.vault_bump = *ctx.bumps.get("game_vault").unwrap();
    game.stake_amount = stake_amount;
    game.pot = 0;
    game.player1_current_bet = 0;
    game.player2_current_bet = 0;
    game.player1_stack = stake_amount;
    game.player2_stack = 0;
    
    // Bonds
    game.player1_bond = bond_amount;
    game.player2_bond = 0;
    
    // Ephemeral keys
    game.player1_ephemeral_pubkey = player1_ephemeral_pubkey;
    game.player2_ephemeral_pubkey = EphemeralPubkey::default();
    
    // Initialize encrypted cards (all zero, will be set when player 2 joins)
    game.encrypted_cards = [EncryptedCard::default(); 9];
    
    // Initialize decryption shares
    game.player1_flop_shares = [EncryptedCard::default(); 3];
    game.player1_turn_share = EncryptedCard::default();
    game.player1_river_share = EncryptedCard::default();
    
    // Initialize hands and community cards
    game.player1_hand = [0u8; 2];
    game.player2_hand = [0u8; 2];
    game.community_cards = [0u8; 5];
    game.community_cards_revealed = 0;
    
    // Game state
    game.stage = GameStage::WaitingForPlayer2;
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
    game.player1_revealed_hand = false;
    game.player2_revealed_hand = false;
    
    // Timing
    game.created_at = clock.unix_timestamp;
    game.last_action_at = clock.unix_timestamp;
    game.action_timeout = 60; // 60 seconds per action
    game.reveal_deadline = 0;
    
    // Result
    game.winner = None;
    game.winning_hand_rank = None;
    
    game.bump = *ctx.bumps.get("game_state").unwrap();
    
    Ok(())
}

#[derive(Accounts)]
#[instruction(stake_amount: u64, player1_ephemeral_pubkey: EphemeralPubkey, game_id: u64)]
pub struct CreateGame<'info> {
    #[account(
        init,
        payer = player1,
        space = GameState::LEN,
        seeds = [
            b"game",
            player1.key().as_ref(),
            &game_id.to_le_bytes()
        ],
        bump
    )]
    pub game_state: Box<Account<'info, GameState>>,
    
    #[account(
        mut,
        seeds = [b"balance", player1.key().as_ref()],
        bump = player1_balance.bump
    )]
    pub player1_balance: Account<'info, PlayerBalance>,
    
    #[account(
        init,
        payer = player1,
        token::mint = usdc_mint,
        token::authority = game_vault,
        seeds = [
            b"game_vault",
            player1.key().as_ref(),
            &game_id.to_le_bytes()
        ],
        bump
    )]
    pub game_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub program_vault: Account<'info, TokenAccount>,
    
    /// CHECK: PDA used for signing token transfers
    #[account(
        seeds = [b"program_vault"],
        bump
    )]
    pub program_vault_authority: AccountInfo<'info>,
    
    pub usdc_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub player1: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

/// Player 2 joins the game
pub fn join_game(
    ctx: Context<JoinGame>,
    player2_ephemeral_pubkey: EphemeralPubkey,
    encrypted_cards: [EncryptedCard; 9],
) -> Result<()> {
    require!(
        player2_ephemeral_pubkey.data != [0u8; 32],
        PokerError::InvalidEphemeralKey
    );
    
    // Validate that encrypted cards are not all zeros
    for card in encrypted_cards.iter() {
        require!(
            card.data != [0u8; 32],
            PokerError::InvalidEncryptedCards
        );
    }
    
    let game = &mut ctx.accounts.game_state;
    let clock = Clock::get()?;
    
    require!(
        game.stage == GameStage::WaitingForPlayer2,
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
    
    // Bond amount (10% of stake)
    let bond_amount = game.stake_amount / 10;
    let total_amount = game.stake_amount + bond_amount;
    
    // Check player has sufficient balance
    let player_balance = &mut ctx.accounts.player2_balance;
    require!(
        player_balance.balance >= total_amount,
        PokerError::InsufficientBalanceToJoin
    );
    
    // Deduct stake + bond from player balance and transfer to game vault
    player_balance.balance = player_balance.balance.checked_sub(total_amount)
        .ok_or(PokerError::InsufficientBalanceToJoin)?;
    
    // Transfer USDC from program vault to game vault
    let seeds = &[
        b"program_vault".as_ref(),
        &[*ctx.bumps.get("program_vault_authority").unwrap()],
    ];
    let signer = &[&seeds[..]];
    
    let cpi_accounts = Transfer {
        from: ctx.accounts.program_vault.to_account_info(),
        to: ctx.accounts.game_vault.to_account_info(),
        authority: ctx.accounts.program_vault_authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, total_amount)?;
    
    // Set player 2 info
    game.player2 = ctx.accounts.player2.key();
    game.player2_ephemeral_pubkey = player2_ephemeral_pubkey;
    game.player2_stack = game.stake_amount;
    game.player2_bond = bond_amount;
    
    // Store the 9 doubly-encrypted cards
    game.encrypted_cards = encrypted_cards;
    
    // Post blinds (in heads-up, button is SB and acts first pre-flop)
    if game.dealer_button == 1 {
        game.player1_current_bet = game.small_blind;
        game.player2_current_bet = game.big_blind;
        game.player1_stack = game.player1_stack.saturating_sub(game.small_blind);
        game.player2_stack = game.player2_stack.saturating_sub(game.big_blind);
        game.pot = game.small_blind + game.big_blind;
        game.current_player = 1; // SB acts first pre-flop
    } else {
        game.player2_current_bet = game.small_blind;
        game.player1_current_bet = game.big_blind;
        game.player2_stack = game.player2_stack.saturating_sub(game.small_blind);
        game.player1_stack = game.player1_stack.saturating_sub(game.big_blind);
        game.pot = game.small_blind + game.big_blind;
        game.current_player = 2;
    }
    
    // Move to pre-flop betting
    game.stage = GameStage::PreFlopBetting;
    game.last_action_at = clock.unix_timestamp;
    
    Ok(())
}

#[derive(Accounts)]
pub struct JoinGame<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
    
    #[account(
        mut,
        seeds = [b"balance", player2.key().as_ref()],
        bump = player2_balance.bump
    )]
    pub player2_balance: Account<'info, PlayerBalance>,
    
    #[account(
        mut,
        constraint = game_vault.key() == game_state.token_vault
    )]
    pub game_vault: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub program_vault: Account<'info, TokenAccount>,
    
    /// CHECK: PDA used for signing token transfers
    #[account(
        seeds = [b"program_vault"],
        bump
    )]
    pub program_vault_authority: AccountInfo<'info>,
    
    #[account(mut)]
    pub player2: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// Reveal community cards - Two step process
/// Step 1: Player 1 submits their decryption shares
/// Step 2: Player 2 submits their decryption shares and plaintext cards, which are then verified
pub fn reveal_community_cards(
    ctx: Context<RevealCommunityCards>,
    decryption_shares: Vec<EncryptedCard>,
    plaintext_cards: Option<Vec<u8>>,
) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let player = ctx.accounts.player.key();
    let clock = Clock::get()?;
    
    let is_player1 = player == game.player1;
    
    // Determine which street we're revealing based on current stage
    match game.stage {
        // FLOP REVEAL
        GameStage::AwaitingFlopReveal => {
            require!(is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 3,
                PokerError::MissingDecryptionShares
            );
            
            // Store Player 1's decryption shares for the flop
            game.player1_flop_shares = [
                decryption_shares[0],
                decryption_shares[1],
                decryption_shares[2],
            ];
            
            // Set deadline for Player 2 to respond
            game.reveal_deadline = clock.unix_timestamp + game.action_timeout;
            game.stage = GameStage::AwaitingPlayer2FlopShare;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::AwaitingPlayer2FlopShare => {
            require!(!is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 3,
                PokerError::MissingDecryptionShares
            );
            require!(
                plaintext_cards.is_some() && plaintext_cards.as_ref().unwrap().len() == 3,
                PokerError::InvalidCommunityCards
            );
            
            let plaintext = plaintext_cards.unwrap();
            
            // Verify each card
            let encrypted_flop = game.get_flop_encrypted_cards();
            for i in 0..3 {
                require!(
                    game.verify_card(plaintext[i], &encrypted_flop[i]),
                    PokerError::CardVerificationFailed
                );
            }
            
            // Store revealed plaintext cards
            game.community_cards[0] = plaintext[0];
            game.community_cards[1] = plaintext[1];
            game.community_cards[2] = plaintext[2];
            game.community_cards_revealed = 3;
            
            // Move to post-flop betting
            // In heads-up, big blind acts first post-flop
            game.current_player = if game.dealer_button == 1 { 2 } else { 1 };
            game.player1_current_bet = 0;
            game.player2_current_bet = 0;
            game.stage = GameStage::PostFlopBetting;
            game.last_action = PlayerActionType::None;
            game.last_action_at = clock.unix_timestamp;
        }
        
        // TURN REVEAL
        GameStage::AwaitingTurnReveal => {
            require!(is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 1,
                PokerError::MissingDecryptionShares
            );
            
            // Store Player 1's decryption share for the turn
            game.player1_turn_share = decryption_shares[0];
            
            // Set deadline for Player 2 to respond
            game.reveal_deadline = clock.unix_timestamp + game.action_timeout;
            game.stage = GameStage::AwaitingPlayer2TurnShare;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::AwaitingPlayer2TurnShare => {
            require!(!is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 1,
                PokerError::MissingDecryptionShares
            );
            require!(
                plaintext_cards.is_some() && plaintext_cards.as_ref().unwrap().len() == 1,
                PokerError::InvalidCommunityCards
            );
            
            let plaintext = plaintext_cards.unwrap();
            let encrypted_turn = game.get_turn_encrypted_card();
            
            // Verify the card
            require!(
                game.verify_card(plaintext[0], &encrypted_turn),
                PokerError::CardVerificationFailed
            );
            
            // Store revealed plaintext card
            game.community_cards[3] = plaintext[0];
            game.community_cards_revealed = 4;
            
            // Move to post-turn betting
            game.current_player = if game.dealer_button == 1 { 2 } else { 1 };
            game.player1_current_bet = 0;
            game.player2_current_bet = 0;
            game.stage = GameStage::PostTurnBetting;
            game.last_action = PlayerActionType::None;
            game.last_action_at = clock.unix_timestamp;
        }
        
        // RIVER REVEAL
        GameStage::AwaitingRiverReveal => {
            require!(is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 1,
                PokerError::MissingDecryptionShares
            );
            
            // Store Player 1's decryption share for the river
            game.player1_river_share = decryption_shares[0];
            
            // Set deadline for Player 2 to respond
            game.reveal_deadline = clock.unix_timestamp + game.action_timeout;
            game.stage = GameStage::AwaitingPlayer2RiverShare;
            game.last_action_at = clock.unix_timestamp;
        }
        
        GameStage::AwaitingPlayer2RiverShare => {
            require!(!is_player1, PokerError::NotYourTurn);
            require!(
                decryption_shares.len() == 1,
                PokerError::MissingDecryptionShares
            );
            require!(
                plaintext_cards.is_some() && plaintext_cards.as_ref().unwrap().len() == 1,
                PokerError::InvalidCommunityCards
            );
            
            let plaintext = plaintext_cards.unwrap();
            let encrypted_river = game.get_river_encrypted_card();
            
            // Verify the card
            require!(
                game.verify_card(plaintext[0], &encrypted_river),
                PokerError::CardVerificationFailed
            );
            
            // Store revealed plaintext card
            game.community_cards[4] = plaintext[0];
            game.community_cards_revealed = 5;
            
            // Move to post-river betting
            game.current_player = if game.dealer_button == 1 { 2 } else { 1 };
            game.player1_current_bet = 0;
            game.player2_current_bet = 0;
            game.stage = GameStage::PostRiverBetting;
            game.last_action = PlayerActionType::None;
            game.last_action_at = clock.unix_timestamp;
        }
        
        _ => {
            return Err(PokerError::InvalidGameStage.into());
        }
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct RevealCommunityCards<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
    
    pub player: Signer<'info>,
}

