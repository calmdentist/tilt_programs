use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
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
    
    // Commit-reveal for randomness
    pub player1_commitment: [u8; 32],
    pub player2_commitment: [u8; 32],
    pub player1_secret_revealed: bool,
    pub player2_secret_revealed: bool,
    pub player1_secret: [u8; 32],
    pub player2_secret: [u8; 32],
    
    // Deck state (52 cards, shuffled using combined secrets)
    pub deck: [u8; 52], // Cards are 0-51 (0-12: clubs, 13-25: diamonds, 26-38: hearts, 39-51: spades)
    pub next_card_index: u8,
    
    // Player hands (2 cards each)
    pub player1_hand: [u8; 2],
    pub player2_hand: [u8; 2],
    
    // Community cards
    pub community_cards: [u8; 5],
    pub community_cards_dealt: u8, // 0, 3 (flop), 4 (turn), 5 (river)
    
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
    
    // Timing
    pub created_at: i64,
    pub last_action_at: i64,
    pub action_timeout: i64, // seconds
    
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
        32 + // player1_commitment
        32 + // player2_commitment
        1 + // player1_secret_revealed
        1 + // player2_secret_revealed
        32 + // player1_secret
        32 + // player2_secret
        52 + // deck
        1 + // next_card_index
        2 + // player1_hand
        2 + // player2_hand
        5 + // community_cards
        1 + // community_cards_dealt
        1 + 1 + 1 + 1 + // stage, current_player, dealer_button, last_action
        8 + // small_blind
        8 + // big_blind
        1 + 1 + 1 + 1 + // player flags
        8 + 8 + 8 + // timing
        33 + // winner (Option<Pubkey>)
        3 + // winning_hand_rank (Option<u16>)
        1; // bump

    pub fn initialize_deck(&mut self, combined_seed: [u8; 32]) {
        // Initialize ordered deck
        for i in 0..52 {
            self.deck[i] = i as u8;
        }
        
        // Fisher-Yates shuffle using the combined seed
        let mut seed = combined_seed;
        for i in (1..52).rev() {
            // Generate random index using keccak hash
            seed = keccak::hash(&seed).to_bytes();
            let j = (u32::from_le_bytes([seed[0], seed[1], seed[2], seed[3]]) as usize) % (i + 1);
            self.deck.swap(i, j);
        }
    }

    pub fn deal_card(&mut self) -> u8 {
        let card = self.deck[self.next_card_index as usize];
        self.next_card_index += 1;
        card
    }

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
}

