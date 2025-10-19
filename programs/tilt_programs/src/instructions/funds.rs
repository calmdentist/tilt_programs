use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use crate::state::*;
use crate::errors::*;

/// Deposit USDC into player balance
pub fn deposit_funds(
    ctx: Context<DepositFunds>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, PokerError::InvalidDepositAmount);
    
    // Transfer USDC from player to program vault
    let cpi_accounts = Transfer {
        from: ctx.accounts.user_token_account.to_account_info(),
        to: ctx.accounts.program_vault.to_account_info(),
        authority: ctx.accounts.authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
    token::transfer(cpi_ctx, amount)?;
    
    // Update player balance
    let balance = &mut ctx.accounts.player_balance;
    balance.balance = balance.balance.checked_add(amount)
        .ok_or(PokerError::InvalidDepositAmount)?;
    
    Ok(())
}

#[derive(Accounts)]
pub struct DepositFunds<'info> {
    #[account(
        mut,
        seeds = [b"balance", authority.key().as_ref()],
        bump = player_balance.bump
    )]
    pub player_balance: Account<'info, PlayerBalance>,
    
    #[account(
        mut,
        constraint = user_token_account.owner == authority.key(),
        constraint = user_token_account.mint == usdc_mint.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = program_vault.mint == usdc_mint.key()
    )]
    pub program_vault: Account<'info, TokenAccount>,
    
    pub usdc_mint: Account<'info, Mint>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

/// Withdraw USDC from player balance
pub fn withdraw_funds(
    ctx: Context<WithdrawFunds>,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, PokerError::InvalidWithdrawalAmount);
    
    let balance = &mut ctx.accounts.player_balance;
    require!(
        balance.balance >= amount,
        PokerError::InsufficientBalance
    );
    
    // Update player balance first
    balance.balance = balance.balance.checked_sub(amount)
        .ok_or(PokerError::InsufficientBalance)?;
    
    // Transfer USDC from program vault to player
    let seeds = &[
        b"program_vault".as_ref(),
        &[*ctx.bumps.get("program_vault_authority").unwrap()],
    ];
    let signer = &[&seeds[..]];
    
    let cpi_accounts = Transfer {
        from: ctx.accounts.program_vault.to_account_info(),
        to: ctx.accounts.user_token_account.to_account_info(),
        authority: ctx.accounts.program_vault_authority.to_account_info(),
    };
    let cpi_program = ctx.accounts.token_program.to_account_info();
    let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
    token::transfer(cpi_ctx, amount)?;
    
    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    #[account(
        mut,
        seeds = [b"balance", authority.key().as_ref()],
        bump = player_balance.bump
    )]
    pub player_balance: Account<'info, PlayerBalance>,
    
    #[account(
        mut,
        constraint = user_token_account.owner == authority.key(),
        constraint = user_token_account.mint == usdc_mint.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    
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
    pub authority: Signer<'info>,
    
    pub token_program: Program<'info, Token>,
}

