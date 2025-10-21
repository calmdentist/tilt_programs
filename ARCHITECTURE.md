# ZK Poker Final Architecture

This document outlines the high-level architecture for an optimistic, provably fair two-player (heads-up) on-chain poker game on Solana. The design supports continuous, multi-hand gameplay and uses Zero-Knowledge Proofs (ZKPs) to guarantee fairness and resolve disputes without a trusted third party.

## Core Principles

1.  **Optimistic Execution**: The on-chain program does not perform cryptographic verification during betting or card reveals. It acts as a state machine, assuming all player actions are valid until a formal dispute is raised.
2.  **Cryptographic Guarantees**: The fairness of the deck (uniqueness and correct shuffling) is **not** optimistic. It is guaranteed at the start of every hand using a mandatory, non-disputable Zero-Knowledge Proof.
3.  **Client-Side Computation**: All intensive cryptographic operations (key generation, encryption, decryption, ZKP generation) are performed on the players' local machines. The blockchain never has access to private keys.
4.  **Dispute Resolution via ZK-SNARKs**: If one player accuses another of cheating during gameplay (e.g., an invalid card reveal), the accused is compelled to produce a ZK-SNARK proving their action was valid. This makes dispute resolution deterministic and trustless.

---

## 1. Cryptographic Primitives

### Commutative Encryption: Paillier Cryptosystem

-   **Why Paillier?**: It is **probabilistic** (non-deterministic), which is critical to prevent brute-force attacks where a player could identify cards by re-encrypting them. It also has homomorphic properties that enable the commutative shuffling effect.
-   **Key Generation**: Each player generates their Paillier keypair off-chain and posts their public key to the on-chain `Game` account once at the start of the match.

### Fairness & Dispute Resolution: ZK-SNARKs (Groth16)

-   **Why Groth16?**: It produces small proofs that are extremely cheap to verify on-chain, thanks to Solana's precompiles for the required elliptic curve operations.
-   **Required ZK Circuits**:
    -   **`ProveCorrectShuffle` (Mandatory)**: This is a prerequisite for every hand. It proves that the doubly-encrypted 52-card deck is a valid permutation and re-encryption of the 52 unique cards committed to by the other player. This prevents duplicate card and deck stacking attacks.
    -   **`ProveCorrectDecryption` (For Disputes)**: Proves that a player correctly used their private key to partially decrypt a card. Used to resolve disputes over card reveals.
    -   **`ProveHandEvaluation` (For Showdown)**: Proves that a player correctly evaluated their best 5-card hand from the 7 cards available to them. This prevents false claims of winning at showdown.

---

## 2. On-Chain Components (Anchor Program)

The program manages a long-running match between two players, with state separated between the overall match and the current hand.

### `Game` Account (The Match Table)

A PDA that stores the persistent state of the match.

```rust
#[account]
pub struct Game {
    pub players: [Pubkey; 2],
    // Cryptographic keys, set once at the start
    pub paillier_pks: [PaillierPublicKey; 2],
    // Player chip stacks, which persist between hands
    pub player_stacks: [u64; 2],
    pub current_hand_id: u64,
    pub game_status: GameStatus, // Enum: Active, Concluded

    // State for the currently active hand
    pub hand: HandState,
}
```

### `HandState` Struct (The Current Hand)

This struct is embedded in the `Game` account and is reset at the beginning of each new hand.

```rust
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct HandState {
    pub state: HandStage, // Enum: Shuffling, Betting, Showdown, Complete, Dispute
    pub dealer_index: u8,
    pub current_turn_index: u8,

    // Cryptographic commitment from Player A (non-dealer) for this hand's deck
    pub deck_merkle_root: [u8; 32],
    // The full 52-card deck, committed to by Player B (dealer) + ZKP
    pub deck: Vec<EncryptedCard>,

    // Betting state for the hand
    pub pot: u64,
    pub bets: [u64; 2],
    pub betting_round: BettingRound,

    // Storage for cards revealed during this hand
    pub revealed_cards: Vec<(u8, PartiallyDecryptedCard)>,

    // Dispute state for the hand
    pub dispute_active: bool,
    pub challenger_index: u8,
    pub disputed_action: DisputedAction,
}
```

### Instructions (Smart Contract Functions)

-   **Match Setup**:
    -   `create_game(player_b, starting_stack)`: Initializes the `Game` account.
    -   `join_game(paillier_pk)`: Player B joins and posts their key. Player A does the same.
-   **Hand Lifecycle**:
    -   `start_new_hand()`: Resets the `HandState`, moves the dealer button, and posts blinds from the main `player_stacks`.
    -   `commit_deck(merkle_root)`: The non-dealer commits to their singly-encrypted deck by posting its Merkle root.
    -   `set_shuffled_deck(deck, proof)`: The dealer submits the final 52-card doubly-encrypted deck along with the mandatory `ProveCorrectShuffle` ZKP. The program verifies the proof on-chain before allowing the hand to proceed.
-   **Gameplay**:
    -   `bet(amount)`: For checking, betting, raising, or folding.
    -   `reveal_card(deck_index, partially_decrypted_card)`: A player reveals a community card. This action is verified off-chain by the opponent using a client-side ZKP exchange.
    -   `claim_winnings(proof)`: At showdown, the winning player claims the pot by providing a `ProveHandEvaluation` ZKP. The program verifies the proof and transfers the pot to the winner's main stack.
-   **Disputes & Match End**:
    -   `raise_dispute(action)`: A player challenges an opponent's action, freezing the hand.
    -   `resolve_dispute(proof)`: The accused must submit the appropriate ZKP to the chain for on-chain verification.
    -   `leave_game()`: A player ends the match and withdraws their stack.

---

## 3. Off-Chain Client & Game Flow

The client application is responsible for all cryptography and for presenting a seamless experience.

### 1. Match Setup
-   Player A calls `create_game`. Player B calls `join_game`. Both post their Paillier keys. Stacks are funded.

### 2. Hand Lifecycle (Loop)
-   **a. Start Hand**: Any player calls `start_new_hand()`. Blinds are posted. The non-dealer is now Player A for this hand, the dealer is Player B.

-   **b. The Secure Shuffle (Mandatory Proofs)**:
    -   **Player A's Client**: Generates a 52-card deck, encrypts it with `pk_A`, builds a Merkle tree from the 52 unique ciphertexts, and calls `commit_deck(merkle_root)`. It then sends the 52 ciphertexts to Player B off-chain.
    -   **Player B's Client**: Receives the 52 ciphertexts. It shuffles and re-encrypts them to create the final 52-card doubly-encrypted deck. It then generates a `ProveCorrectShuffle` ZKP. Finally, it calls `set_shuffled_deck(deck, proof)`. The on-chain program verifies the proof. **If the proof is invalid, the transaction fails, and the hand cannot begin.**

-   **c. Gameplay (Optimistic with Off-Chain Verification)**:
    -   Betting proceeds via the `bet` instruction.
    -   When a card needs to be revealed, the responsible player's client computes the partial decryption and a `ProveCorrectDecryption` ZKP. It sends both to the opponent **off-chain**.
    -   The opponent's client verifies the proof. If valid, the game continues. If invalid, the client immediately calls `raise_dispute` to escalate the issue to the on-chain program.

-   **d. Showdown**:
    -   The player claiming to be the winner calls `claim_winnings`, providing the `ProveHandEvaluation` ZKP. The contract verifies the proof and awards the pot.

-   **e. Loop**: A new hand can be started by calling `start_new_hand()`.

### 3. Match End
-   A player calls `leave_game` to withdraw their funds and conclude the match.