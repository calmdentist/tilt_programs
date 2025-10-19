use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use crate::state::*;
use crate::errors::*;
use crate::poker;

/// Resolve the game and determine winner
pub fn resolve_game(mut ctx: Context<ResolveGame>) -> Result<()> {
    let game = &mut ctx.accounts.game_state;
    
    require!(
        game.stage == GameStage::Showdown,
        PokerError::InvalidGameStage
    );
    
    // Evaluate both hands
    let player1_score = poker::find_best_hand(&game.player1_hand, &game.community_cards).1;
    let player2_score = poker::find_best_hand(&game.player2_hand, &game.community_cards).1;
    
    // Store pot value before modification
    let pot_amount = game.pot;
    let stake = game.stake_amount as i64;
    
    // Determine winner and winnings
    let (p1_win, p2_win) = if player1_score > player2_score {
        game.winner = Some(game.player1);
        game.winning_hand_rank = Some((player1_score >> 20) as u16);
        (pot_amount, 0)
    } else if player2_score > player1_score {
        game.winner = Some(game.player2);
        game.winning_hand_rank = Some((player2_score >> 20) as u16);
        (0, pot_amount)
    } else {
        game.winner = None;
        (pot_amount / 2, pot_amount / 2)
    };
    
    game.stage = GameStage::Completed;
    
    // Transfer pot from game vault to program vault
    if pot_amount > 0 {
        transfer_pot_to_vault(&ctx, pot_amount)?;
    }
    
    // Update stats and balances
    update_player_stats(&mut ctx.accounts, p1_win, p2_win, stake)?;
    
    Ok(())
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
    
    pub token_program: Program<'info, Token>,
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

