# ZKPoker: Architecture Overview

This document outlines the high-level architecture for an optimistic, provably fair two-player (heads-up) on-chain poker game on Solana. The design supports continuous, multi-hand gameplay and uses Zero-Knowledge Proofs (ZKPs) to guarantee fairness and resolve disputes without a trusted third party.

## Core Principles

1.  **Optimistic Execution**: The on-chain program does not perform cryptographic verification during betting or card reveals. It acts as a state machine, assuming all player actions are valid until a formal dispute is raised.
2.  **Cryptographic Guarantees**: The fairness of the deck (uniqueness and correct shuffling) is **not** optimistic. It is guaranteed at the start of every hand using a mandatory, non-disputable Zero-Knowledge Proof.
3.  **Client-Side Computation**: All intensive cryptographic operations (key generation, encryption, decryption, ZKP generation) are performed on the players' local machines. The blockchain never has access to private keys.
4.  **Dispute Resolution via ZK-SNARKs**: If one player accuses another of cheating during gameplay (e.g., an invalid card reveal), the accused is compelled to produce a ZK-SNARK proving their action was valid. This makes dispute resolution deterministic and trustless.

---

## 1. Cryptographic Primitives

### Commutative Encryption: Paillier Cryptosystem

-   **Why Paillier?**: It is **probabilistic** (non-deterministic), which is critical to prevent brute-force attacks where a player could identify cards by re-encrypting all 52 cards. It also has homomorphic properties that enable the commutative shuffling effect.
-   **Key Generation**: Each player generates their Paillier keypair off-chain and posts their public key to the on-chain `Game` account once at the start of the match.

### Fairness & Dispute Resolution: ZK-SNARKs (Groth16)

-   **Why Groth16?**: It produces small proofs that are extremely cheap to verify on-chain, thanks to Solana's precompiles for the required elliptic curve operations.
-   **Required ZK Circuits**:
    -   **`ProveCorrectDeckCreation` (Mandatory)**: Proves the initial encrypted deck contains exactly 52 unique cards. Verified on-chain immediately in `create_hand`.
    -   **`ProveCorrectReshuffle` (Optimistic)**: Proves the deck was correctly re-shuffled and re-encrypted. Stored in `join_hand` and verified on-chain only in a dispute.
    -   **`ProveCorrectDecryption` (Optimistic)**: Proves a card was correctly decrypted. Stored in `player_action` or `showdown` and verified on-chain only in a dispute.

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
    // Cryptographic commitment from Player B (dealer) for the final deck
    pub doubly_encrypted_deck_merkle_root: [u8; 32],

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

*Client applications can bundle instructions (e.g., `create_game` + `create_hand`) into a single transaction for better UX.*

-   **Match Setup**:
    -   `create_game(paillier_pk, starting_stack, opponent: Option<Pubkey>)`: Initializes the `Game` account, sets Player A's Paillier key, and funds their stack. The optional `opponent` pubkey can create an invite-only game.
    -   `join_game(paillier_pk)`: Player B joins a game, setting their Paillier key and funding their stack.
-   **Hand Lifecycle**:
    -   `create_hand(merkle_root, proof)`: The non-dealer starts a new hand, posting blinds and committing to their deck. The `ProveCorrectDeckCreation` ZKP is **verified on-chain immediately**.
    -   `join_hand(deck_root, reshuffle_proof, p1_cards, decryption_proof)`: The dealer joins the hand by submitting the **merkle root** of the final deck, revealing Player A's cards, and posting their blind. The ZKPs are stored optimistically.
-   **Gameplay**:
    -   `player_action(action, card_reveal_data)`: A single instruction for all betting moves. If the action triggers a card reveal, `card_reveal_data` must be provided. This data now includes: the decrypted card, its `ProveCorrectDecryption` ZKP, the original **doubly-encrypted card**, and its **merkle proof** to verify it against the stored deck root.
-   **Showdown & Resolution**:
    -   `showdown(pocket_cards, proof, encrypted_cards, merkle_proofs)`: Each player calls this to reveal their plaintext pocket cards. They must also provide the original doubly-encrypted cards and their merkle proofs.
    -   `resolve_hand()`: After both players have revealed their hands, either player can call this to trigger the on-chain hand evaluation and pot distribution.
-   **Disputes & Match End**:
    -   `claim_timeout(disputed_action)`: A player challenges an opponent's optimistic action. This triggers the on-chain verification of the relevant stored ZKP. If the proof fails, the challenger wins the pot.
    -   `leave_game()`: A player gracefully exits the match and withdraws their chip stack.

---

## 3. Off-Chain Client & Game Flow

The client application is responsible for all cryptography and for presenting a seamless experience.

### 1. Match Setup
-   Player A calls `create_game`. Player B calls `join_game`. Both post their Paillier keys. Stacks are funded.

### 2. Hand Lifecycle (Loop)
-   **a. Start Hand**: Any player calls `start_new_hand()`. Blinds are posted. The non-dealer is now Player A for this hand, the dealer is Player B.

-   **b. The Secure Shuffle (One Mandatory, One Optimistic Proof)**:
    -   **Player A's Client (Creator)**: Generates a 52-card deck, encrypts it with `pk_A`, and builds a Merkle tree. It then generates a `ProveCorrectDeckCreation` ZKP. It calls `commit_deck(merkle_root, proof)`. The on-chain program verifies this proof immediately. **If the proof is invalid, the transaction fails, and the hand cannot begin.** Player A then sends the 52 ciphertexts to Player B off-chain.
    -   **Player B's Client (Dealer)**: Receives the 52 ciphertexts and validates them against the on-chain Merkle root. It then shuffles, re-encrypts them, computes a merkle root of this final deck, and generates a `ProveCorrectReshuffle` ZKP. It calls `join_hand(deck_root, proof, ...)` to commit to the final deck.

    **c. Gameplay (Optimistic with On-Chain Verification)**:
    -   Betting proceeds via the `player_action` instruction.
    -   When a card is revealed, the player submits the decrypted data, the ZKP, the original encrypted card, and its merkle proof. The on-chain program **must** verify the merkle proof immediately to ensure the card is from the committed deck before accepting the action. The ZKP remains optimistic.

    **d. Showdown**:
    -   Players reveal their pocket cards via the `showdown` instruction, providing all data required to prove the cards are legitimate (plaintext, ZKP, encrypted card, merkle proof).
    -   If no disputes are raised, either player calls `resolve_hand()` to trigger the final on-chain evaluation.

    **e. Loop**: A new hand can be started.

### 3. Match End
-   A player calls `leave_game` to withdraw their funds and conclude the match.