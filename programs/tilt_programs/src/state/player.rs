use anchor_lang::prelude::*;

/// Player account that persists across games
#[account]
pub struct PlayerAccount {
    pub authority: Pubkey,
    pub total_hands_played: u64,
    pub total_hands_won: u64,
    pub total_winnings: i64, // Can be negative
    pub bump: u8,
}

impl PlayerAccount {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        8 + // total_hands_played
        8 + // total_hands_won
        8 + // total_winnings
        1; // bump
}

/// Player balance account for USDC deposits
#[account]
pub struct PlayerBalance {
    pub authority: Pubkey,
    pub balance: u64, // USDC balance in smallest units (6 decimals)
    pub bump: u8,
}

impl PlayerBalance {
    pub const LEN: usize = 8 + // discriminator
        32 + // authority
        8 + // balance
        1; // bump
}

