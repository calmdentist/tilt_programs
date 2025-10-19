use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use crate::state::*;
use crate::errors::*;

/// Create a new game
pub fn create_game(
    ctx: Context<CreateGame>,
    stake_amount: u64,
    commitment: [u8; 32],
    game_id: u64,
) -> Result<()> {
    require!(commitment != [0u8; 32], PokerError::ZeroCommitment);
    require!(stake_amount > 0, PokerError::InvalidBetAmount);
    
    // Check player has sufficient balance
    let player_balance = &mut ctx.accounts.player1_balance;
    require!(
        player_balance.balance >= stake_amount,
        PokerError::InsufficientBalanceToJoin
    );
    
    // Deduct stake from player balance and transfer to game vault
    player_balance.balance = player_balance.balance.checked_sub(stake_amount)
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
    token::transfer(cpi_ctx, stake_amount)?;
    
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
    game.player1_stack = stake_amount; // Player 1 starts with full stake
    game.player2_stack = 0; // Player 2 will be set when they join
    
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
#[instruction(stake_amount: u64, commitment: [u8; 32], game_id: u64)]
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
    pub game_state: Account<'info, GameState>,
    
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
    
    // Check player has sufficient balance
    let player_balance = &mut ctx.accounts.player2_balance;
    require!(
        player_balance.balance >= game.stake_amount,
        PokerError::InsufficientBalanceToJoin
    );
    
    // Deduct stake from player balance and transfer to game vault
    player_balance.balance = player_balance.balance.checked_sub(game.stake_amount)
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
    token::transfer(cpi_ctx, game.stake_amount)?;
    
    game.player2 = ctx.accounts.player2.key();
    game.player2_commitment = commitment;
    game.player2_stack = game.stake_amount; // Player 2 starts with full stake
    game.stage = GameStage::WaitingForReveals;
    game.last_action_at = clock.unix_timestamp;
    
    Ok(())
}

#[derive(Accounts)]
pub struct JoinGame<'info> {
    #[account(mut)]
    pub game_state: Account<'info, GameState>,
    
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

