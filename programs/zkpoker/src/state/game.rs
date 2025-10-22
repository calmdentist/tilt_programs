use anchor_lang::prelude::*;
use super::types::*;

/// The main Game account - persists across multiple hands
/// This is a PDA that stores the long-running match state between two players
#[account]
pub struct Game {
    /// The two players in this game
    pub players: [Pubkey; 2],
    
    /// Paillier public keys for each player (set once at game creation/join)
    pub paillier_pks: [PaillierPublicKey; 2],
    
    /// Player chip stacks (persist across hands)
    pub player_stacks: [u64; 2],
    
    /// Current hand number (increments with each new hand)
    pub current_hand_id: u64,
    
    /// Overall game status
    pub game_status: GameStatus,
    
    /// Token vault for this game
    pub token_vault: Pubkey,
    pub vault_bump: u8,
    
    /// Blinds configuration (can be updated between hands)
    pub small_blind: u64,
    pub big_blind: u64,
    
    /// Timing configuration
    pub action_timeout: i64, // seconds
    
    /// Invite-only game (if Some, only this pubkey can join)
    pub invited_opponent: Option<Pubkey>,
    
    /// State for the currently active hand
    pub hand: HandState,
    
    /// Bump seed for PDA
    pub bump: u8,
    pub last_action_timestamp: i64, // Timestamp of the last move in the match
}

/// HandState - embedded in Game, reset at the start of each new hand
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct HandState {
    /// Current stage of the hand
    pub stage: HandStage,
    
    /// Dealer index (0 or 1) - rotates each hand
    pub dealer_index: u8,
    
    /// Current turn index (0 or 1) - whose turn to act
    pub current_turn_index: u8,
    
    /// Timestamp for the current player's action
    pub action_deadline: i64,
    
    /// Merkle root of the non-dealer's singly-encrypted deck
    /// This is the commitment submitted in create_hand
    pub deck_merkle_root: [u8; 32],
    
    /// The full 52-card doubly-encrypted deck (submitted by dealer in join_hand)
    pub doubly_encrypted_deck_merkle_root: [u8; 32],
    
    /// Betting state for this hand
    pub pot: u64,
    pub bets: [u64; 2], // Current bets for each player in this round
    pub betting_round: BettingRound,
    
    /// Revealed cards during this hand
    /// Stores (card_index, partially_decrypted_card_data) tuples
    /// Used for progressive card reveals (singly-decrypted, then fully-decrypted)
    pub revealed_cards: [Option<(u8, PartiallyDecryptedCard)>; 9],
    
    /// Fully decrypted community cards (plaintext)
    /// Indices: [flop1, flop2, flop3, turn, river]
    pub community_cards: [Option<u8>; 5],
    
    /// Fully decrypted pocket cards for each player (revealed at showdown)
    /// Each player has 2 pocket cards
    pub pocket_cards: [Option<[u8; 2]>; 2],
    
    /// ZK-SNARK proofs submitted during this hand (stored optimistically)
    pub stored_proofs: [Option<StoredProof>; 20],
    
    /// Dispute state
    pub dispute_active: bool,
    pub challenger_index: u8, // 0 or 1
    pub disputed_action: DisputedAction,
    
    /// Player flags for this hand
    pub player_folded: [bool; 2],
    pub player_all_in: [bool; 2],
    pub player_revealed_showdown: [bool; 2],
    
    /// Timing
    pub hand_started_at: i64,
    pub last_action_at: i64,
    
    /// Hand result (set after resolve_hand)
    pub winner: Option<u8>, // 0 or 1, or None for split pot
    pub winning_hand_rank: Option<HandRank>,
}

/// Stored ZK-SNARK proof with metadata
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct StoredProof {
    pub proof_type: ProofType,
    pub submitter_index: u8, // 0 or 1
    pub proof: ZkProof,
    pub submitted_at: i64,
}

/// Types of ZK-SNARK proofs
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum ProofType {
    /// Mandatory proof: deck contains 52 unique cards (verified immediately)
    DeckCreation,
    /// Optimistic: deck was correctly reshuffled and re-encrypted
    Reshuffle,
    /// Optimistic: card was correctly decrypted (specify card index)
    CardDecryption { card_index: u8 },
    /// Optimistic: pocket cards were correctly revealed at showdown
    ShowdownReveal { player_index: u8 },
}

impl Game {
    /// Calculate space needed for Game account
    /// Note: This is a rough estimate, actual size will vary based on Vec lengths
    pub const BASE_LEN: usize = 8 + // discriminator
        (32 * 2) + // players
        (4 + 256 + 4 + 257) * 2 + // paillier_pks (rough estimate for Vec<u8>)
        (8 * 2) + // player_stacks
        8 + // current_hand_id
        1 + // game_status
        32 + // token_vault
        1 + // vault_bump
        8 + // small_blind
        8 + // big_blind
        8 + // action_timeout
        (1 + 32) + // invited_opponent (Option<Pubkey>)
        1 + // bump
        8 + // last_action_timestamp
        4096; // HandState (we'll allocate a large buffer for the embedded state)
    
    /// Initialize a new hand within this game
    pub fn init_new_hand(&mut self, clock: &Clock) {
        // Rotate dealer
        let new_dealer_index = if self.current_hand_id == 0 {
            0 // First hand, player 0 is dealer
        } else {
            1 - self.hand.dealer_index // Alternate dealer
        };
        
        // Non-dealer acts first pre-flop in our model
        let non_dealer_index = 1 - new_dealer_index;
        
        self.hand = HandState {
            stage: HandStage::WaitingForHandCreation,
            dealer_index: new_dealer_index,
            current_turn_index: non_dealer_index,
            action_deadline: clock.unix_timestamp + self.action_timeout,
            deck_merkle_root: [0u8; 32],
            doubly_encrypted_deck_merkle_root: [0u8; 32],
            pot: 0,
            bets: [0, 0],
            betting_round: BettingRound::PreFlop,
            revealed_cards: [None; 9],
            community_cards: [None; 5],
            pocket_cards: [None; 2],
            stored_proofs: [None; 20],
            dispute_active: false,
            challenger_index: 0,
            disputed_action: DisputedAction::None,
            player_folded: [false, false],
            player_all_in: [false, false],
            player_revealed_showdown: [false, false],
            hand_started_at: clock.unix_timestamp,
            last_action_at: clock.unix_timestamp,
            winner: None,
            winning_hand_rank: None,
        };
        
        self.current_hand_id += 1;
    }
    
    /// Get player pubkey by index (0 or 1)
    pub fn get_player(&self, index: u8) -> Result<Pubkey> {
        match index {
            0 => Ok(self.players[0]),
            1 => Ok(self.players[1]),
            _ => err!(GameError::InvalidPlayerIndex),
        }
    }
    
    /// Get player index (0 or 1) from pubkey
    pub fn get_player_index(&self, player: &Pubkey) -> Result<u8> {
        if player == &self.players[0] {
            Ok(0)
        } else if player == &self.players[1] {
            Ok(1)
        } else {
            err!(GameError::InvalidPlayer)
        }
    }
    
    /// Check if it's the specified player's turn
    pub fn is_player_turn(&self, player: &Pubkey) -> Result<bool> {
        let player_index = self.get_player_index(player)?;
        Ok(player_index == self.hand.current_turn_index)
    }
    
    /// Check if action timeout has been exceeded
    pub fn is_timeout_exceeded(&self, clock: &Clock) -> bool {
        clock.unix_timestamp > self.hand.last_action_at + self.action_timeout
    }
    
    /// Post blinds at the start of a hand
    pub fn post_blinds(&mut self) -> Result<()> {
        let dealer_index = self.hand.dealer_index as usize;
        let non_dealer_index = (1 - self.hand.dealer_index) as usize;
        
        // In heads-up: dealer posts small blind, non-dealer posts big blind
        require!(
            self.player_stacks[dealer_index] >= self.small_blind,
            GameError::InsufficientStack
        );
        require!(
            self.player_stacks[non_dealer_index] >= self.big_blind,
            GameError::InsufficientStack
        );
        
        // Deduct blinds from stacks
        self.player_stacks[dealer_index] -= self.small_blind;
        self.player_stacks[non_dealer_index] -= self.big_blind;
        
        // Add to pot and track bets
        self.hand.pot = self.small_blind + self.big_blind;
        self.hand.bets[dealer_index] = self.small_blind;
        self.hand.bets[non_dealer_index] = self.big_blind;
        
        Ok(())
    }
    
    /// Check if betting round is complete
    pub fn is_betting_round_complete(&self) -> bool {
        // If someone folded, round is complete
        if self.hand.player_folded[0] || self.hand.player_folded[1] {
            return true;
        }
        
        // If both players are all-in, round is complete
        if self.hand.player_all_in[0] && self.hand.player_all_in[1] {
            return true;
        }
        
        // If bets are equal and both players have acted
        self.hand.bets[0] == self.hand.bets[1]
    }
    
    /// Advance to next betting round
    pub fn advance_betting_round(&mut self) {
        // Move pot forward, reset bets
        self.hand.bets = [0, 0];
        
        // Advance the betting round
        self.hand.betting_round = match self.hand.betting_round {
            BettingRound::PreFlop => BettingRound::Flop,
            BettingRound::Flop => BettingRound::Turn,
            BettingRound::Turn => BettingRound::River,
            BettingRound::River => BettingRound::River, // Stay at river
        };
        
        // Update stage
        self.hand.stage = match self.hand.betting_round {
            BettingRound::PreFlop => HandStage::PreFlopBetting,
            BettingRound::Flop => HandStage::FlopBetting,
            BettingRound::Turn => HandStage::TurnBetting,
            BettingRound::River => HandStage::RiverBetting,
        };
    }
    
    /// Switch turn to the other player
    pub fn switch_turn(&mut self) {
        self.hand.current_turn_index = 1 - self.hand.current_turn_index;
    }
    
    /// Store a ZK proof optimistically (not verified immediately)
    pub fn store_proof(
        &mut self,
        proof_type: ProofType,
        submitter_index: u8,
        proof: ZkProof,
        clock: &Clock,
    ) -> Result<()> {
        let new_proof = StoredProof {
            proof_type,
            submitter_index,
            proof,
            submitted_at: clock.unix_timestamp,
        };
        
        for entry in self.hand.stored_proofs.iter_mut() {
            if entry.is_none() {
                *entry = Some(new_proof);
                return Ok(());
            }
        }
        
        err!(GameError::MaxProofsReached)
    }
    
    /// Reveal a community card (store partially decrypted version)
    pub fn reveal_card(
        &mut self,
        card_index: u8,
        partially_decrypted: PartiallyDecryptedCard,
    ) -> Result<()> {
        for entry in self.hand.revealed_cards.iter_mut() {
            if entry.is_none() {
                *entry = Some((card_index, partially_decrypted));
                return Ok(());
            }
        }
        err!(GameError::MaxCardsReached)
    }
    
    /// Finalize a community card (store fully decrypted plaintext)
    pub fn finalize_community_card(&mut self, position: usize, card: u8) -> Result<()> {
        require!(position < 5, GameError::InvalidCardPosition);
        self.hand.community_cards[position] = Some(card);
        Ok(())
    }
    
    /// Reveal pocket cards at showdown
    pub fn reveal_pocket_cards(&mut self, player_index: u8, cards: [u8; 2]) -> Result<()> {
        require!(player_index < 2, GameError::InvalidPlayerIndex);
        self.hand.pocket_cards[player_index as usize] = Some(cards);
        self.hand.player_revealed_showdown[player_index as usize] = true;
        Ok(())
    }
    
    /// Award pot to winner
    pub fn award_pot(&mut self, winner_index: u8) -> Result<()> {
        require!(winner_index < 2, GameError::InvalidPlayerIndex);
        self.player_stacks[winner_index as usize] += self.hand.pot;
        self.hand.pot = 0;
        Ok(())
    }
    
    /// Split pot (tie)
    pub fn split_pot(&mut self) {
        let half_pot = self.hand.pot / 2;
        self.player_stacks[0] += half_pot;
        self.player_stacks[1] += half_pot;
        // Handle odd chip
        if self.hand.pot % 2 == 1 {
            // Give odd chip to dealer (standard poker rule)
            self.player_stacks[self.hand.dealer_index as usize] += 1;
        }
        self.hand.pot = 0;
    }
}

impl Default for Game {
    fn default() -> Self {
        Game {
            players: [Pubkey::default(); 2],
            paillier_pks: [PaillierPublicKey::default(); 2],
            player_stacks: [0; 2],
            current_hand_id: 0,
            game_status: GameStatus::Pending,
            token_vault: Pubkey::default(),
            vault_bump: 0,
            small_blind: 0,
            big_blind: 0,
            action_timeout: 0,
            invited_opponent: None,
            hand: HandState::default(),
            bump: 0,
            last_action_timestamp: 0,
        }
    }
}

/// Custom error codes for game logic
#[error_code]
pub enum GameError {
    #[msg("Invalid player index")]
    InvalidPlayerIndex,
    #[msg("Invalid player")]
    InvalidPlayer,
    #[msg("Insufficient stack")]
    InsufficientStack,
    #[msg("Invalid card position")]
    InvalidCardPosition,
    #[msg("Maximum number of proofs have been stored for this hand")]
    MaxProofsReached,
    #[msg("Maximum number of cards have been revealed for this hand")]
    MaxCardsReached,
}
