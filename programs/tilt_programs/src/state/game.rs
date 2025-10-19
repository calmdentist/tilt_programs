use anchor_lang::prelude::*;
use super::types::*;

/// Represents a single poker hand/game between two players
#[account]
pub struct GameState {
    pub game_id: u64,
    pub player1: Pubkey,
    pub player2: Pubkey,
    
    // Token vault for this game
    pub token_vault: Pubkey,
    pub vault_bump: u8,
    
    // Stake and pot
    pub stake_amount: u64,
    pub pot: u64,
    pub player1_current_bet: u64,
    pub player2_current_bet: u64,
    
    // Player chip stacks (remaining balance in this game)
    pub player1_stack: u64,
    pub player2_stack: u64,
    
    // Player bonds (for griefing prevention)
    pub player1_bond: u64,
    pub player2_bond: u64,
    
    // Ephemeral keys for Pohlig-Hellman encryption
    pub player1_ephemeral_pubkey: EphemeralPubkey,
    pub player2_ephemeral_pubkey: EphemeralPubkey,
    
    // Encrypted cards (9 total: 2 per player + 5 community)
    // Indices: 0-1 = Player 1 pocket cards, 2-3 = Player 2 pocket cards, 4-8 = Community cards
    pub encrypted_cards: [EncryptedCard; 9],
    
    // Decryption shares for community cards (stored during two-step reveal)
    pub player1_flop_shares: [EncryptedCard; 3],  // Flop (3 cards)
    pub player1_turn_share: EncryptedCard,         // Turn (1 card)
    pub player1_river_share: EncryptedCard,        // River (1 card)
    
    // Revealed plaintext cards
    pub player1_hand: [u8; 2],  // Only revealed at showdown
    pub player2_hand: [u8; 2],  // Only revealed at showdown
    pub community_cards: [u8; 5],  // Revealed progressively
    pub community_cards_revealed: u8, // 0, 3 (flop), 4 (turn), 5 (river)
    
    // Game state
    pub stage: GameStage,
    pub current_player: u8, // 1 or 2
    pub dealer_button: u8, // 1 or 2 (small blind is dealer button in heads-up)
    pub last_action: PlayerActionType,
    
    // Positions (for heads-up: button is small blind and acts first pre-flop)
    pub small_blind: u64,
    pub big_blind: u64,
    
    // Player states
    pub player1_folded: bool,
    pub player2_folded: bool,
    pub player1_all_in: bool,
    pub player2_all_in: bool,
    pub player1_revealed_hand: bool,  // For showdown tracking
    pub player2_revealed_hand: bool,  // For showdown tracking
    
    // Timing
    pub created_at: i64,
    pub last_action_at: i64,
    pub action_timeout: i64, // seconds
    pub reveal_deadline: i64, // Specific deadline for two-step reveals
    
    // Result
    pub winner: Option<Pubkey>,
    pub winning_hand_rank: Option<u16>,
    
    pub bump: u8,
}

impl GameState {
    pub const LEN: usize = 8 + // discriminator
        8 + // game_id
        32 + // player1
        32 + // player2
        32 + // token_vault
        1 + // vault_bump
        8 + // stake_amount
        8 + // pot
        8 + // player1_current_bet
        8 + // player2_current_bet
        8 + // player1_stack
        8 + // player2_stack
        8 + // player1_bond
        8 + // player2_bond
        32 + // player1_ephemeral_pubkey
        32 + // player2_ephemeral_pubkey
        (32 * 9) + // encrypted_cards (9 cards)
        (32 * 3) + // player1_flop_shares (3 cards)
        32 + // player1_turn_share
        32 + // player1_river_share
        2 + // player1_hand
        2 + // player2_hand
        5 + // community_cards
        1 + // community_cards_revealed
        1 + 1 + 1 + 1 + // stage, current_player, dealer_button, last_action
        8 + // small_blind
        8 + // big_blind
        1 + 1 + 1 + 1 + 1 + 1 + // player flags (folded, all_in, revealed_hand x2)
        8 + 8 + 8 + 8 + // timing (created_at, last_action_at, action_timeout, reveal_deadline)
        33 + // winner (Option<Pubkey>)
        3 + // winning_hand_rank (Option<u16>)
        1; // bump

    pub fn is_betting_round_complete(&self) -> bool {
        // Both players have acted and bets are equal, or someone folded/is all-in
        if self.player1_folded || self.player2_folded {
            return true;
        }
        
        if self.player1_all_in || self.player2_all_in {
            return true;
        }

        // Check if both players have equal bets and both have acted
        self.player1_current_bet == self.player2_current_bet && 
        self.last_action != PlayerActionType::None
    }

    pub fn get_other_player(&self, player: &Pubkey) -> Pubkey {
        if player == &self.player1 {
            self.player2
        } else {
            self.player1
        }
    }

    pub fn is_player_turn(&self, player: &Pubkey) -> bool {
        if self.current_player == 1 {
            player == &self.player1
        } else {
            player == &self.player2
        }
    }

    /// Verify encrypted card matches plaintext by re-encrypting with both player keys
    /// This is the core verification logic for the mental poker protocol
    pub fn verify_card(
        &self,
        _plaintext_card: u8,
        _encrypted_card: &EncryptedCard,
    ) -> bool {
        // This is a placeholder for the actual Pohlig-Hellman verification
        // In the real implementation, this would:
        // 1. Encrypt plaintext_card with player1's public key
        // 2. Encrypt that result with player2's public key
        // 3. Compare with the stored encrypted_card
        // For now, we'll implement this with the num-bigint library later
        
        // TODO: Implement actual Pohlig-Hellman verification
        // let encrypted_once = modpow(plaintext_card, player1_pubkey, PRIME);
        // let encrypted_twice = modpow(encrypted_once, player2_pubkey, PRIME);
        // encrypted_twice == encrypted_card.data
        
        true // Placeholder
    }
    
    /// Get encrypted cards for flop (indices 4, 5, 6)
    pub fn get_flop_encrypted_cards(&self) -> [EncryptedCard; 3] {
        [
            self.encrypted_cards[4],
            self.encrypted_cards[5],
            self.encrypted_cards[6],
        ]
    }
    
    /// Get encrypted card for turn (index 7)
    pub fn get_turn_encrypted_card(&self) -> EncryptedCard {
        self.encrypted_cards[7]
    }
    
    /// Get encrypted card for river (index 8)
    pub fn get_river_encrypted_card(&self) -> EncryptedCard {
        self.encrypted_cards[8]
    }
    
    /// Get encrypted cards for player 1's hand (indices 0, 1)
    pub fn get_player1_encrypted_cards(&self) -> [EncryptedCard; 2] {
        [self.encrypted_cards[0], self.encrypted_cards[1]]
    }
    
    /// Get encrypted cards for player 2's hand (indices 2, 3)
    pub fn get_player2_encrypted_cards(&self) -> [EncryptedCard; 2] {
        [self.encrypted_cards[2], self.encrypted_cards[3]]
    }
}

