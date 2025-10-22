use anchor_lang::prelude::*;

/// Overall game status (persists across hands)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Active,
    Concluded,
}

/// Hand stage within a single hand
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum HandStage {
    /// Waiting for non-dealer to create hand and commit to deck
    WaitingForHandCreation,
    /// Waiting for dealer to join hand with shuffled deck
    WaitingForDealerJoin,
    /// Pre-flop betting in progress
    PreFlopBetting,
    /// Flop betting in progress
    FlopBetting,
    /// Turn betting in progress
    TurnBetting,
    /// River betting in progress
    RiverBetting,
    /// Showdown: players revealing pocket cards
    Showdown,
    /// Hand completed, ready for resolution
    Complete,
    /// A dispute has been raised
    Dispute,
}

/// Betting round identifiers
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum BettingRound {
    PreFlop,
    Flop,
    Turn,
    River,
}

/// Player action types
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PlayerActionType {
    None,
    Fold,
    Check,
    Call,
    Raise,
    AllIn,
}

/// Types of disputed actions for claim_timeout
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum DisputedAction {
    None,
    /// Dispute the initial deck creation proof
    DeckCreation,
    /// Dispute the reshuffle proof submitted during join_hand
    Reshuffle,
    /// Dispute a card decryption proof (specify card index)
    CardDecryption { card_index: u8 },
    /// Dispute opponent's showdown reveal
    ShowdownReveal { player_index: u8 },
}

/// Card utilities
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Card(pub u8); // 0-51

impl Card {
    pub fn rank(self) -> u8 {
        self.0 % 13 // 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A
    }

    pub fn suit(self) -> u8 {
        self.0 / 13 // 0=clubs, 1=diamonds, 2=hearts, 3=spades
    }

    pub fn rank_value(self) -> u8 {
        // Returns value for comparison (2=2, 3=3, ..., 10=10, J=11, Q=12, K=13, A=14)
        self.rank() + 2
    }
}

/// Encrypted card representation (for Paillier cryptosystem)
/// Paillier ciphertext is typically 2048 bits, but we'll use 256 bytes to be safe
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Default)]
pub struct EncryptedCard {
    pub data: Vec<u8>, // Variable-length encrypted value (Paillier ciphertext)
}

/// Paillier public key (n, g where n = p*q for large primes p,q)
/// For 1024-bit security, n is 2048 bits (256 bytes)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Default)]
pub struct PaillierPublicKey {
    pub n: Vec<u8>,  // Modulus (typically 256 bytes for 2048-bit)
    pub g: Vec<u8>,  // Generator (typically 257 bytes)
}

/// Partially decrypted card (singly decrypted, still encrypted with one player's key)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Default)]
pub struct PartiallyDecryptedCard {
    pub data: Vec<u8>,
}

/// ZK-SNARK proof storage (Groth16 proofs are small: ~128-256 bytes)
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Default)]
pub struct ZkProof {
    pub proof_data: Vec<u8>, // Serialized Groth16 proof
}

/// Storage for card reveal with its proof
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct CardReveal {
    pub card_index: u8,              // Index in deck (0-51)
    pub encrypted_card: EncryptedCard, // The encrypted/partially decrypted card
    pub proof: ZkProof,               // Proof of correct decryption
}

/// Hand rankings for poker evaluation
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
    RoyalFlush = 9,
}

