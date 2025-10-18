# Tilt Poker Protocol - Technical Architecture

## Table of Contents
1. [Overview](#overview)
2. [Smart Contract Architecture](#smart-contract-architecture)
3. [State Management](#state-management)
4. [Randomness & Provable Fairness](#randomness--provable-fairness)
5. [Game Flow](#game-flow)
6. [Poker Hand Evaluation](#poker-hand-evaluation)
7. [Security Considerations](#security-considerations)
8. [Gas Optimization](#gas-optimization)

---

## Overview

Tilt is a fully on-chain poker protocol designed for Solana. It implements heads-up (2-player) Texas Hold'em with a fast-fold format, where players are matched for single hands and then re-matched with new opponents.

### Key Features
- **Provably Fair**: Commit-reveal scheme ensures neither player can manipulate the deck
- **On-Chain**: All game logic executes on-chain with verifiable state
- **Fast-Fold**: Quick player rotation prevents collusion
- **Timeout Protection**: Time limits prevent griefing
- **Stats Tracking**: Persistent player statistics across games

---

## Smart Contract Architecture

### Module Structure

```
programs/tilt_programs/src/
├── lib.rs              # Program entrypoint and instruction handlers
├── state.rs            # State structs and account definitions
├── instructions.rs     # Instruction logic implementation
├── errors.rs           # Custom error definitions
└── poker.rs           # Hand evaluation algorithms
```

### Core Components

#### 1. **lib.rs** - Program Entrypoint
Defines the public API and instruction handlers. All instructions are declared here and delegate to the `instructions` module.

#### 2. **state.rs** - State Management
- `PlayerAccount`: Persistent player stats
- `GameState`: Single game instance state
- `Card`: Card representation utilities
- Enums for game stages and actions

#### 3. **instructions.rs** - Business Logic
Implements all game actions:
- Player initialization
- Game creation and joining
- Secret reveal for randomness
- Card dealing
- Betting actions
- Street advancement
- Game resolution
- Timeout handling

#### 4. **poker.rs** - Hand Evaluation
Pure functions for evaluating poker hands:
- Hand ranking calculation
- Best 5-card hand selection from 7 cards
- Tie-breaking logic

#### 5. **errors.rs** - Error Handling
Custom error codes for all failure cases

---

## State Management

### PlayerAccount (PDA)
```rust
pub struct PlayerAccount {
    pub authority: Pubkey,           // Player's wallet
    pub total_hands_played: u64,     // Lifetime hand count
    pub total_hands_won: u64,        // Lifetime wins
    pub total_winnings: i64,         // Net profit/loss
    pub bump: u8,                    // PDA bump seed
}
```

**PDA Derivation**: `["player", player_pubkey]`

### GameState (PDA)
```rust
pub struct GameState {
    // Game identification
    pub game_id: u64,
    pub player1: Pubkey,
    pub player2: Pubkey,
    
    // Pot and stakes
    pub stake_amount: u64,
    pub pot: u64,
    pub player1_current_bet: u64,
    pub player2_current_bet: u64,
    
    // Commit-reveal randomness
    pub player1_commitment: [u8; 32],
    pub player2_commitment: [u8; 32],
    pub player1_secret_revealed: bool,
    pub player2_secret_revealed: bool,
    pub player1_secret: [u8; 32],
    pub player2_secret: [u8; 32],
    
    // Deck and cards
    pub deck: [u8; 52],
    pub next_card_index: u8,
    pub player1_hand: [u8; 2],
    pub player2_hand: [u8; 2],
    pub community_cards: [u8; 5],
    pub community_cards_dealt: u8,
    
    // Game flow
    pub stage: GameStage,
    pub current_player: u8,
    pub dealer_button: u8,
    pub last_action: PlayerActionType,
    
    // Blinds
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
    pub action_timeout: i64,
    
    // Result
    pub winner: Option<Pubkey>,
    pub winning_hand_rank: Option<u16>,
    
    pub bump: u8,
}
```

**PDA Derivation**: `["game", player1_pubkey, timestamp]`

### Game Stages
```rust
pub enum GameStage {
    WaitingForPlayers,      // Initial state
    WaitingForCommitments,  // (Not used - direct to reveals)
    WaitingForReveals,      // Both players joined, awaiting secrets
    PreFlop,                // Pre-flop betting
    Flop,                   // Flop betting (3 community cards)
    Turn,                   // Turn betting (4 community cards)
    River,                  // River betting (5 community cards)
    Showdown,               // Evaluate hands
    Completed,              // Game finished
}
```

---

## Randomness & Provable Fairness

### Commit-Reveal Scheme

The protocol uses a 2-party commit-reveal scheme to generate verifiable randomness:

#### Phase 1: Commitment
1. **Player 1** generates a random 32-byte secret `S1`
2. **Player 1** computes commitment: `C1 = keccak256(S1)`
3. **Player 1** creates game with `C1`
4. **Player 2** generates random 32-byte secret `S2`
5. **Player 2** computes commitment: `C2 = keccak256(S2)`
6. **Player 2** joins game with `C2`

#### Phase 2: Reveal
7. **Player 1** reveals `S1` on-chain
8. Program verifies: `keccak256(S1) == C1`
9. **Player 2** reveals `S2` on-chain
10. Program verifies: `keccak256(S2) == C2`

#### Phase 3: Deck Generation
11. Combine secrets: `combined = S1 ⊕ S2` (XOR)
12. Generate seed: `seed = keccak256(combined)`
13. Shuffle deck using Fisher-Yates with deterministic randomness from seed

### Security Properties

- **Neither player can predict the deck**: Each player only knows their own secret before reveal
- **Neither player can manipulate the deck**: The deck depends on both secrets
- **Verifiable**: Anyone can verify the deck by checking the revealed secrets
- **No third party needed**: No oracle or external randomness source required

### Fisher-Yates Shuffle

```rust
pub fn initialize_deck(&mut self, combined_seed: [u8; 32]) {
    // Initialize ordered deck (0-51)
    for i in 0..52 {
        self.deck[i] = i as u8;
    }
    
    // Fisher-Yates shuffle using the combined seed
    let mut seed = combined_seed;
    for i in (1..52).rev() {
        // Generate random index using keccak hash
        seed = keccak::hash(&seed).to_bytes();
        let j = (u32::from_le_bytes([seed[0], seed[1], seed[2], seed[3]]) 
                 as usize) % (i + 1);
        self.deck.swap(i, j);
    }
}
```

---

## Game Flow

### Complete Game Lifecycle

```
1. create_game
   ├─> Player 1 creates game with commitment
   └─> State: WaitingForPlayers

2. join_game
   ├─> Player 2 joins with commitment
   └─> State: WaitingForReveals

3. reveal_secret (Player 1)
   ├─> Verify secret matches commitment
   └─> Store secret

4. reveal_secret (Player 2)
   ├─> Verify secret matches commitment
   ├─> Combine secrets and shuffle deck
   ├─> Post blinds
   └─> State: PreFlop

5. deal_initial
   ├─> Deal 2 cards to each player
   └─> Cards remain on-chain (encrypted visibility optional)

6. player_action (Pre-flop)
   ├─> Player actions: Fold/Check/Call/Raise
   └─> Betting round continues until complete

7. advance_street
   ├─> Deal 3 cards (flop)
   └─> State: Flop

8. player_action (Flop)
   └─> Betting round

9. advance_street
   ├─> Deal 1 card (turn)
   └─> State: Turn

10. player_action (Turn)
    └─> Betting round

11. advance_street
    ├─> Deal 1 card (river)
    └─> State: River

12. player_action (River)
    └─> Betting round

13. advance_street
    └─> State: Showdown

14. resolve_game
    ├─> Evaluate both hands
    ├─> Determine winner
    ├─> Update player stats
    └─> State: Completed
```

### Heads-Up Specific Rules

In heads-up poker:
- **Button is small blind**: The dealer button posts the small blind
- **Pre-flop action**: Small blind (button) acts first pre-flop
- **Post-flop action**: Big blind acts first on all post-flop streets
- **Button rotates**: The button alternates between players each hand (not implemented in current single-hand protocol)

---

## Poker Hand Evaluation

### Card Representation

Cards are represented as `u8` values from 0-51:

```
Rank calculation: rank = card % 13
- 0 = 2
- 1 = 3
- ...
- 8 = 10
- 9 = Jack
- 10 = Queen
- 11 = King
- 12 = Ace

Suit calculation: suit = card / 13
- 0 = Clubs ♣
- 1 = Diamonds ♦
- 2 = Hearts ♥
- 3 = Spades ♠
```

### Hand Evaluation Algorithm

The `evaluate_hand()` function returns a 32-bit score:
```
Bits 20-23: Hand rank (0-9)
Bits 0-19:  Tie-breaker values (card ranks)
```

Higher score = better hand.

#### Hand Rankings (High to Low)
1. **Royal Flush** (9): A-K-Q-J-10 of same suit
2. **Straight Flush** (8): 5 consecutive ranks, same suit
3. **Four of a Kind** (7): 4 cards of same rank
4. **Full House** (6): 3 of a kind + pair
5. **Flush** (5): 5 cards of same suit
6. **Straight** (4): 5 consecutive ranks
7. **Three of a Kind** (3): 3 cards of same rank
8. **Two Pair** (2): 2 different pairs
9. **One Pair** (1): 2 cards of same rank
10. **High Card** (0): No other hand made

#### Best Hand Selection

The `find_best_hand()` function:
1. Takes 7 cards (2 hole + 5 community)
2. Generates all 21 possible 5-card combinations
3. Evaluates each combination
4. Returns the best hand and its score

```rust
pub fn find_best_hand(hole_cards: &[u8; 2], community_cards: &[u8; 5]) 
    -> ([u8; 5], u32)
{
    // Generate all C(7,5) = 21 combinations
    // Return combination with highest score
}
```

---

## Security Considerations

### 1. Randomness
- ✅ **Commit-reveal scheme** prevents manipulation
- ✅ **Both players contribute entropy** (no single point of control)
- ✅ **On-chain verification** of commitments
- ⚠️ Note: Players can see each other's hole cards on-chain (future: use encryption)

### 2. Timing Attacks
- ✅ **60-second action timeout** prevents griefing
- ✅ **claim_timeout** allows non-timing-out player to win
- ✅ **Timestamps verified** using Solana's Clock

### 3. State Manipulation
- ✅ **PDA-based accounts** prevent unauthorized changes
- ✅ **Signer checks** on all player actions
- ✅ **Stage validation** prevents invalid state transitions
- ✅ **Bet validation** prevents invalid raises/calls

### 4. Economic Security
- ✅ **Fixed stake amounts** prevent mismatches
- ✅ **Blinds posted automatically** after reveal
- ✅ **Pot accounting** tracks all bets
- ⚠️ Future: Implement token-based stakes with escrow

### 5. Replay Protection
- ✅ **Unique game PDAs** using player + timestamp
- ✅ **Cannot reuse old commitments** (one-time use)

---

## Gas Optimization

### Account Size Optimization
- Fixed-size arrays for deck and cards
- Bit-packed enums where possible
- Minimal dynamic allocation

### Instruction Optimization
- Batch operations (e.g., post blinds in reveal_secret)
- Avoid unnecessary recomputations
- Pre-computed PDAs in client

### State Updates
- Only update changed fields
- Use saturating arithmetic to prevent overflows
- Minimal state cloning

---

## Future Enhancements

### 1. Card Privacy
- Implement homomorphic encryption or ZK proofs for hole cards
- Reveal only at showdown or when needed

### 2. Token Integration
- SPL token support for stakes
- Escrow accounts for pot management
- Rake collection mechanism

### 3. Multi-Table Support
- Tournament structures
- Sit-and-go formats
- Cash game tables

### 4. Advanced Features
- Side pots for all-in scenarios with multiple players
- Player reputation/rating system
- Hand history storage (using Solana logs or external indexer)
- Automated matchmaking contract

### 5. UI/UX
- Real-time updates via WebSocket
- Table animations
- Chat and emotes
- Mobile support

---

## Testing Strategy

### Unit Tests
- Poker hand evaluation correctness
- Card shuffling randomness distribution
- Bet amount calculations

### Integration Tests
- Complete game flows
- Error handling scenarios
- Timeout edge cases
- State transition validations

### Security Tests
- Commitment verification
- Authorization checks
- Invalid action prevention
- Race condition testing

---

## Deployment Checklist

- [ ] Audit smart contracts
- [ ] Test on devnet extensively
- [ ] Verify randomness quality
- [ ] Load test with multiple concurrent games
- [ ] Document all edge cases
- [ ] Prepare upgrade path
- [ ] Set up monitoring and alerts
- [ ] Create incident response plan

---

## References

- [Solana Anchor Framework](https://www.anchor-lang.com/)
- [Commit-Reveal Schemes](https://en.wikipedia.org/wiki/Commitment_scheme)
- [Fisher-Yates Shuffle](https://en.wikipedia.org/wiki/Fisher%E2%80%93Yates_shuffle)
- [Texas Hold'em Rules](https://en.wikipedia.org/wiki/Texas_hold_%27em)
- [Poker Hand Rankings](https://en.wikipedia.org/wiki/List_of_poker_hands)

