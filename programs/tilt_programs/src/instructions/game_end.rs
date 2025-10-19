use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::*;
use crate::errors::*;
use crate::poker;

/// Resolve hand at showdown - Two step process
/// Step 1: First player reveals their pocket cards
/// Step 2: Second player reveals their pocket cards, then winner is determined
pub fn resolve_hand(mut ctx: Context<ResolveGame>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    let player = ctx.accounts.player.key();
    let clock = Clock::get()?;
    
    require!(
        game.stage == GameStage::Showdown || game.stage == GameStage::AwaitingPlayer2ShowdownReveal,
        PokerError::InvalidGameStage
    );
    
    let is_player1 = player == game.player1;
    
    if game.stage == GameStage::Showdown {
        // First player revealing their hand
        require!(
            !game.player1_revealed_hand && !game.player2_revealed_hand,
            PokerError::AlreadyRevealedHand
        );
        
        // Mark player as having revealed
        if is_player1 {
            game.player1_revealed_hand = true;
        } else {
            game.player2_revealed_hand = true;
        }
        
        // Set deadline for other player to reveal
        game.reveal_deadline = clock.unix_timestamp + game.action_timeout;
        game.stage = GameStage::AwaitingPlayer2ShowdownReveal;
        game.last_action_at = clock.unix_timestamp;
        
        return Ok(());
    }
    
    if game.stage == GameStage::AwaitingPlayer2ShowdownReveal {
        // Second player revealing their hand
        require!(
            (is_player1 && !game.player1_revealed_hand) || (!is_player1 && !game.player2_revealed_hand),
            PokerError::AlreadyRevealedHand
        );
        
        // Mark player as having revealed
        if is_player1 {
            game.player1_revealed_hand = true;
        } else {
            game.player2_revealed_hand = true;
        }
        
        // Verify both players' pocket cards against encrypted versions
        let p1_encrypted = game.get_player1_encrypted_cards();
        let p2_encrypted = game.get_player2_encrypted_cards();
        
        // Verify player 1's cards
        require!(
            game.verify_card(game.player1_hand[0], &p1_encrypted[0]),
            PokerError::CardVerificationFailed
        );
        require!(
            game.verify_card(game.player1_hand[1], &p1_encrypted[1]),
            PokerError::CardVerificationFailed
        );
        
        // Verify player 2's cards
        require!(
            game.verify_card(game.player2_hand[0], &p2_encrypted[0]),
            PokerError::CardVerificationFailed
        );
        require!(
            game.verify_card(game.player2_hand[1], &p2_encrypted[1]),
            PokerError::CardVerificationFailed
        );
        
        // Evaluate both hands
        let player1_score = poker::find_best_hand(&game.player1_hand, &game.community_cards).1;
        let player2_score = poker::find_best_hand(&game.player2_hand, &game.community_cards).1;
        
        // Store pot value before modification
        let pot_amount = game.pot;
        let stake = game.stake_amount as i64;
        
        // Return bonds to both players
        let total_amount = pot_amount + game.player1_bond + game.player2_bond;
        
        // Determine winner and winnings (including bond returns)
        let (p1_win, p2_win) = if player1_score > player2_score {
            game.winner = Some(game.player1);
            game.winning_hand_rank = Some((player1_score >> 20) as u16);
            (pot_amount + game.player1_bond, game.player2_bond)
        } else if player2_score > player1_score {
            game.winner = Some(game.player2);
            game.winning_hand_rank = Some((player2_score >> 20) as u16);
            (game.player1_bond, pot_amount + game.player2_bond)
        } else {
            game.winner = None;
            let pot_split = pot_amount / 2;
            (pot_split + game.player1_bond, pot_split + game.player2_bond)
        };
        
        game.stage = GameStage::Finished;
        
        // Transfer total amount from game vault to program vault
        if total_amount > 0 {
            transfer_pot_to_vault(&ctx, total_amount)?;
        }
        
        // Update stats and balances
        update_player_stats(&mut ctx.accounts, p1_win, p2_win, stake)?;
    }
    
    Ok(())
}

/// Compatibility alias for resolve_hand
pub fn resolve_game(ctx: Context<ResolveGame>) -> Result<()> {
    resolve_hand(ctx)
}

fn transfer_pot_to_vault(ctx: &Context<ResolveGame>, pot_amount: u64) -> Result<()> {
    let game = &ctx.accounts.game_state;
    let game_id = game.game_id;
    let player1_key = game.player1;
    let vault_bump = game.vault_bump;
    let seeds = &[
        b"game_vault".as_ref(),
        player1_key.as_ref(),
        &game_id.to_le_bytes(),
        &[vault_bump],
    ];
    let signer = &[&seeds[..]];
    
    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        Transfer {
            from: ctx.accounts.game_vault.to_account_info(),
            to: ctx.accounts.program_vault.to_account_info(),
            authority: ctx.accounts.game_vault.to_account_info(),
        },
        signer
    );
    token::transfer(cpi_ctx, pot_amount)
}

fn update_player_stats(
    accounts: &mut ResolveGame,
    p1_win: u64,
    p2_win: u64,
    stake: i64,
) -> Result<()> {
    accounts.player1_account.total_hands_played += 1;
    accounts.player2_account.total_hands_played += 1;
    
    if p1_win > 0 {
        accounts.player1_account.total_hands_won += 1;
        accounts.player1_account.total_winnings = accounts.player1_account
            .total_winnings.saturating_add(p1_win as i64).saturating_sub(stake);
        accounts.player2_account.total_winnings = accounts.player2_account
            .total_winnings.saturating_sub(stake);
        accounts.player1_balance.balance = accounts.player1_balance.balance.saturating_add(p1_win);
    } else if p2_win > 0 {
        accounts.player2_account.total_hands_won += 1;
        accounts.player2_account.total_winnings = accounts.player2_account
            .total_winnings.saturating_add(p2_win as i64).saturating_sub(stake);
        accounts.player1_account.total_winnings = accounts.player1_account
            .total_winnings.saturating_sub(stake);
        accounts.player2_balance.balance = accounts.player2_balance.balance.saturating_add(p2_win);
    } else {
        // Split pot
        accounts.player1_account.total_winnings = accounts.player1_account
            .total_winnings.saturating_add(p1_win as i64).saturating_sub(stake);
        accounts.player2_account.total_winnings = accounts.player2_account
            .total_winnings.saturating_add(p2_win as i64).saturating_sub(stake);
        accounts.player1_balance.balance = accounts.player1_balance.balance.saturating_add(p1_win);
        accounts.player2_balance.balance = accounts.player2_balance.balance.saturating_add(p2_win);
    }
    
    Ok(())
}

#[derive(Accounts)]
pub struct ResolveGame<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
    
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
    
    #[account(
        mut,
        seeds = [b"balance", game_state.player1.as_ref()],
        bump = player1_balance.bump
    )]
    pub player1_balance: Account<'info, PlayerBalance>,
    
    #[account(
        mut,
        seeds = [b"balance", game_state.player2.as_ref()],
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
    
    pub player: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// Claim timeout win if opponent doesn't act
/// Winner receives the pot + their bond back + opponent's bond (penalty)
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
    
    // Verify it's not the claiming player's turn (or their reveal deadline)
    // In reveal stages, check against reveal_deadline
    let is_timeout = match game.stage {
        GameStage::AwaitingPlayer2FlopShare |
        GameStage::AwaitingPlayer2TurnShare |
        GameStage::AwaitingPlayer2RiverShare |
        GameStage::AwaitingPlayer2ShowdownReveal => {
            clock.unix_timestamp > game.reveal_deadline
        }
        _ => {
            !game.is_player_turn(&player) && elapsed > game.action_timeout
        }
    };
    
    require!(is_timeout, PokerError::TimeoutNotReached);
    
    // Award win to the player who didn't timeout
    game.winner = Some(player);
    
    // Winner gets pot + their bond + opponent's bond (as penalty)
    let is_player1 = player == game.player1;
    let winner_amount = game.pot + game.player1_bond + game.player2_bond;
    
    if is_player1 {
        ctx.accounts.player1_balance.balance = ctx.accounts.player1_balance.balance
            .saturating_add(winner_amount);
    } else {
        ctx.accounts.player2_balance.balance = ctx.accounts.player2_balance.balance
            .saturating_add(winner_amount);
    }
    
    game.stage = GameStage::Finished;
    
    Ok(())
}

#[derive(Accounts)]
pub struct ClaimTimeout<'info> {
    #[account(mut)]
    pub game_state: Box<Account<'info, GameState>>,
    
    #[account(
        mut,
        seeds = [b"balance", game_state.player1.as_ref()],
        bump = player1_balance.bump
    )]
    pub player1_balance: Account<'info, PlayerBalance>,
    
    #[account(
        mut,
        seeds = [b"balance", game_state.player2.as_ref()],
        bump = player2_balance.bump
    )]
    pub player2_balance: Account<'info, PlayerBalance>,
    
    pub player: Signer<'info>,
}

