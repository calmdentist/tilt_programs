use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use super::types::*;
use crypto_bigint::{U256, U512, Encoding, NonZero};

// 256-bit safe prime for Pohlig-Hellman cipher
// This is 2^256 - 189 in big-endian byte format
// Chosen for: (1) Large enough for security, (2) Small enough for on-chain compute
const PRIME_BYTES: [u8; 32] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x43,
];

/// Helper function to get the prime modulus as U256
fn get_prime() -> U256 {
    U256::from_be_bytes(PRIME_BYTES)
}

/// Helper function to perform modular exponentiation: base^exp mod modulus
/// Uses square-and-multiply algorithm with crypto-bigint
fn modpow(base: &U256, exp: &U256, modulus: &U256) -> U256 {
    let modulus_nz = NonZero::new(*modulus).unwrap();
    let modulus_wide = U512::from(modulus);
    let modulus_wide_nz = NonZero::new(modulus_wide).unwrap();
    
    let mut result = U256::ONE;
    let mut base_pow = *base % modulus_nz;
    
    // Process each bit of the exponent (LSB to MSB)
    for i in 0..256 {
        let word_index = i / 64;
        let bit_offset = i % 64;
        let bit_is_set = (exp.as_words()[word_index] >> bit_offset) & 1 == 1;
        
        if bit_is_set {
            // result = (result * base_pow) mod modulus
            // mul_wide returns (high, low) as (U256, U256)
            let (high, low) = result.mul_wide(&base_pow);
            // Combine into U512
            let product_wide = concat_u256_to_u512(&low, &high);
            result = (product_wide % modulus_wide_nz).resize();
        }
        
        // base_pow = (base_pow * base_pow) mod modulus
        let (high, low) = base_pow.mul_wide(&base_pow);
        let square_wide = concat_u256_to_u512(&low, &high);
        base_pow = (square_wide % modulus_wide_nz).resize();
    }
    
    result
}

/// Helper to concatenate two U256 values into a U512 (low || high)
fn concat_u256_to_u512(low: &U256, high: &U256) -> U512 {
    let low_words = low.as_words();
    let high_words = high.as_words();
    U512::from_words([
        low_words[0],
        low_words[1],
        low_words[2],
        low_words[3],
        high_words[0],
        high_words[1],
        high_words[2],
        high_words[3],
    ])
}

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
    
    // Merkle root of Player 1's singly-encrypted deck (52 cards)
    // This commits Player 1 to their shuffled deck before Player 2 acts
    pub deck_merkle_root: [u8; 32],
    
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
        32 + // deck_merkle_root
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
        plaintext_card: u8,
        encrypted_card: &EncryptedCard,
    ) -> bool {
        // Get the prime modulus
        let prime = get_prime();
        
        // Convert plaintext card to U256 (cards are 0-51)
        // Map card to a value in the valid range (2 to prime-1)
        // We add 2 to ensure we're never 0 or 1
        let plaintext = U256::from_u64(plaintext_card as u64 + 2);
        
        // Convert player keys from bytes to U256 (big-endian)
        let player1_key = U256::from_be_bytes(self.player1_ephemeral_pubkey.data);
        let player2_key = U256::from_be_bytes(self.player2_ephemeral_pubkey.data);
        
        // Validate that keys are in valid range (2 to prime-1)
        let two = U256::from_u64(2);
        if player1_key < two || player1_key >= prime {
            return false;
        }
        if player2_key < two || player2_key >= prime {
            return false;
        }
        
        // First encryption: plaintext^player1_key mod prime
        let encrypted_once = modpow(&plaintext, &player1_key, &prime);
        
        // Second encryption: encrypted_once^player2_key mod prime
        // This is the commutative property: (m^a)^b = (m^b)^a mod p
        let encrypted_twice = modpow(&encrypted_once, &player2_key, &prime);
        
        // Convert the stored encrypted card to U256 for comparison
        let expected_encrypted = U256::from_be_bytes(encrypted_card.data);
        
        // Verify that our computed encryption matches the stored value
        encrypted_twice == expected_encrypted
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
    
    /// Verify a Merkle proof for a card in the deck
    /// Proves that a singly-encrypted card was part of Player 1's committed deck
    pub fn verify_merkle_proof(
        card: &EncryptedCard,
        proof: &[[u8; 32]],
        root: &[u8; 32],
        index: usize,
    ) -> bool {
        // Start with the leaf hash (hash of the card data)
        let mut current_hash = keccak::hash(&card.data).to_bytes();
        let mut current_index = index;
        
        // Process each proof element
        for proof_element in proof {
            // Determine if we hash (current || proof) or (proof || current)
            // based on whether current_index is even or odd
            let combined = if current_index % 2 == 0 {
                // Current is left child, proof is right sibling
                let mut data = [0u8; 64];
                data[..32].copy_from_slice(&current_hash);
                data[32..].copy_from_slice(proof_element);
                data
            } else {
                // Current is right child, proof is left sibling
                let mut data = [0u8; 64];
                data[..32].copy_from_slice(proof_element);
                data[32..].copy_from_slice(&current_hash);
                data
            };
            
            // Hash the combined data
            current_hash = keccak::hash(&combined).to_bytes();
            
            // Move up the tree
            current_index /= 2;
        }
        
        // Check if the computed root matches the stored root
        current_hash == *root
    }
}

// Helper functions for Pohlig-Hellman encryption (can be used by clients)
impl GameState {
    /// Encrypt a card value using a public key
    /// This performs: card^key mod prime
    pub fn encrypt_card(card: u8, public_key: &EphemeralPubkey) -> EncryptedCard {
        let prime = get_prime();
        
        // Map card value (0-51) to valid range (2 to prime-1)
        let plaintext = U256::from_u64(card as u64 + 2);
        let key = U256::from_be_bytes(public_key.data);
        
        // Perform modular exponentiation
        let encrypted = modpow(&plaintext, &key, &prime);
        
        // Convert result to 32-byte array (big-endian)
        let result = encrypted.to_be_bytes();
        
        EncryptedCard { data: result }
    }
    
    /// Encrypt already-encrypted bytes (for second layer of encryption)
    /// This performs: encrypted_value^key mod prime
    /// Used when Player 2 encrypts Player 1's already-encrypted cards
    pub fn encrypt_card_bytes(encrypted_bytes: &[u8; 32], public_key: &EphemeralPubkey) -> EncryptedCard {
        let prime = get_prime();
        
        // Convert encrypted bytes to U256
        let encrypted_value = U256::from_be_bytes(*encrypted_bytes);
        let key = U256::from_be_bytes(public_key.data);
        
        // Perform modular exponentiation on the already-encrypted value
        let double_encrypted = modpow(&encrypted_value, &key, &prime);
        
        // Convert result to 32-byte array (big-endian)
        let result = double_encrypted.to_be_bytes();
        
        EncryptedCard { data: result }
    }
    
    /// Decrypt a card using a private key (for off-chain use only)
    /// This computes the modular multiplicative inverse: card = encrypted^(key^-1) mod prime
    /// Note: This requires computing the private key inverse, which is expensive
    pub fn decrypt_card(encrypted: &EncryptedCard, private_key: &[u8; 32]) -> Option<u8> {
        let prime = get_prime();
        let encrypted_val = U256::from_be_bytes(encrypted.data);
        let key = U256::from_be_bytes(*private_key);
        
        // Compute modular inverse of the key using Fermat's little theorem
        // For prime p: key^-1 = key^(p-2) mod p
        let prime_minus_two = prime.wrapping_sub(&U256::from_u64(2));
        let inv_key = modpow(&key, &prime_minus_two, &prime);
        
        // Decrypt: plaintext = encrypted^(key^-1) mod prime
        let plaintext = modpow(&encrypted_val, &inv_key, &prime);
        
        // Convert back to card value (subtract 2 to get 0-51)
        // Extract the low 64 bits
        let card_plus_2 = plaintext.as_words()[0];
        if card_plus_2 >= 2 && card_plus_2 <= 53 {
            return Some((card_plus_2 - 2) as u8);
        }
        
        None
    }
}

