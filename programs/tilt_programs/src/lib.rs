use anchor_lang::prelude::*;

declare_id!("5mqXj7QV7SGLsJ3n6UCeau4sd7aDtaBH7E9WaHoiGKHA");

mod state;
mod instructions;
mod errors;
mod poker;

use state::*;
use instructions::*;

#[program]
pub mod tilt_programs {
    use super::*;

    /// Initialize a player account
    pub fn initialize_player(ctx: Context<InitializePlayer>) -> Result<()> {
        instructions::initialize_player(ctx)
    }

    /// Initialize a player balance account
    pub fn initialize_balance(ctx: Context<InitializeBalance>) -> Result<()> {
        instructions::initialize_balance(ctx)
    }

    /// Deposit USDC into player balance
    pub fn deposit_funds(ctx: Context<DepositFunds>, amount: u64) -> Result<()> {
        instructions::deposit_funds(ctx, amount)
    }

    /// Withdraw USDC from player balance
    pub fn withdraw_funds(ctx: Context<WithdrawFunds>, amount: u64) -> Result<()> {
        instructions::withdraw_funds(ctx, amount)
    }

    /// Create a new game with player 1's commitment
    pub fn create_game(
        ctx: Context<CreateGame>,
        stake_amount: u64,
        commitment: [u8; 32],
        game_id: u64,
    ) -> Result<()> {
        instructions::create_game(ctx, stake_amount, commitment, game_id)
    }

    /// Player 2 joins the game with their commitment
    pub fn join_game(
        ctx: Context<JoinGame>,
        commitment: [u8; 32],
    ) -> Result<()> {
        instructions::join_game(ctx, commitment)
    }

    /// Both players reveal their secrets to generate the deck
    pub fn reveal_secret(
        ctx: Context<RevealSecret>,
        secret: [u8; 32],
    ) -> Result<()> {
        instructions::reveal_secret(ctx, secret)
    }

    /// Deal initial cards (pocket cards) after both secrets revealed
    pub fn deal_initial(ctx: Context<DealInitial>) -> Result<()> {
        instructions::deal_initial(ctx)
    }

    /// Player action: fold, check, call, or raise
    pub fn player_action(
        ctx: Context<PlayerAction>,
        action: PlayerActionType,
        raise_amount: Option<u64>,
    ) -> Result<()> {
        instructions::player_action(ctx, action, raise_amount)
    }

    /// Advance to next street (flop, turn, river) or showdown
    pub fn advance_street(ctx: Context<AdvanceStreet>) -> Result<()> {
        instructions::advance_street(ctx)
    }

    /// Resolve the game and distribute winnings
    pub fn resolve_game(ctx: Context<ResolveGame>) -> Result<()> {
        instructions::resolve_game(ctx)
    }

    /// Claim timeout win if opponent doesn't act within time limit
    pub fn claim_timeout(ctx: Context<ClaimTimeout>) -> Result<()> {
        instructions::claim_timeout(ctx)
    }
}
