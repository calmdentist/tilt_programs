use anchor_lang::prelude::*;
use crate::state::*;

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

/// Initialize a player balance account
pub fn initialize_balance(ctx: Context<InitializeBalance>) -> Result<()> {
    let balance = &mut ctx.accounts.player_balance;
    balance.authority = ctx.accounts.authority.key();
    balance.balance = 0;
    balance.bump = *ctx.bumps.get("player_balance").unwrap();
    
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeBalance<'info> {
    #[account(
        init,
        payer = authority,
        space = PlayerBalance::LEN,
        seeds = [b"balance", authority.key().as_ref()],
        bump
    )]
    pub player_balance: Account<'info, PlayerBalance>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

