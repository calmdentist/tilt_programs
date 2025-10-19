# zkPoker: Permissionless, provably fair poker on Solana

## 1. Overview

This document outlines a cryptographic protocol for implementing a secure, trustless, two-player (heads-up) poker game on the Solana blockchain. The core challenge is to manage the deck and player hands without a central, trusted dealer, ensuring that no player has an unfair advantage or access to hidden information.

This is achieved using a "mental poker" approach, where players collaboratively shuffle and encrypt the deck.

## 2. The Protocol

The protocol is divided into three main phases: Setup, Gameplay, and Verification.

### Phase 1: Game Setup & Shuffling

The goal of this phase is to create a shuffled, encrypted deck that is committed to the on-chain game state.

1.  **Key Generation**: Both players (Player 1 and Player 2) generate ephemeral keypairs for a **commutative encryption scheme**. The recommended scheme is Pohlig-Hellman (modular exponentiation). Each player submits their public key to the program.
2.  **Player 1's Shuffle & Encrypt**: Player 1 takes a canonical 52-card deck, shuffles it locally, and encrypts each card with their public key. They pass this singly-encrypted deck to Player 2.
3.  **Player 2's Shuffle & Encrypt**: Player 2 takes the singly-encrypted deck, shuffles it again locally, and applies a second layer of encryption. From this final deck, Player 2 selects the 9 cards necessary for the hand (2 pocket cards for each player, 5 community cards) and submits only these 9 doubly-encrypted cards to the program's on-chain account. This avoids storing the entire deck on-chain.

At the end of this phase, the 9 cards for the hand are committed to the chain, and no single party knows their plaintext values.

### Phase 2: Gameplay

During gameplay, cards are revealed to individual players (pocket cards) or both players (community cards) without exposing the rest of the deck.

1.  **Dealing Pocket Cards**: To deal a card to Player 1, Player 2 first provides the singly-decrypted card to Player 1 (decrypted with Player 2's key). Player 1 can then fully decrypt it locally. This process is non-custodial and happens via communication between the players' clients. The program only needs to know which encrypted cards were assigned to which player.
2.  **Dealing Community Cards**: To reveal a community card (e.g., on the flop), a sequential, timed reveal process is initiated on-chain. Player 1 must first submit their decryption of the card(s). Then, Player 2 has a fixed time limit to submit their decryption.
    *   **Progressive Verification**: As soon as the plaintext for a community card (or set of cards, like the flop) is revealed to the program, the program **immediately verifies it** by re-encrypting the plaintext with the players' public keys and checking it against the stored encrypted version. This distributes the computational load and ensures fraud is detected instantly.
    *   If a player fails to act within their time limit, they forfeit the pot (see Liveness and Security).
3.  **Wagering**: All betting actions happen on-chain, with funds being transferred into a shared pot account, as is standard.

### Phase 3: Showdown & Verification

This is the final on-chain phase. When a hand concludes and goes to a showdown, the program verifies the players' pocket cards to determine the winner.

1.  **Card Reveal**: Players reveal their plaintext pocket cards to the program.
2.  **On-Chain Verification**: The program verifies only the pocket cards revealed by the players. The community cards have already been verified progressively throughout the hand.
    *   For a revealed pocket card `C`, the program calculates `Verified_C = Encrypt_P2(Encrypt_P1(C))`.
    *   It then compares this result with the original doubly-encrypted card assigned to that player.
    *   If they match, the player's hand is proven to be legitimate.
3.  **Winner Determination**: After verifying the pocket cards, the program combines them with the already-verified community cards and runs standard poker hand-ranking logic to determine the winner and execute the pot distribution.

## 3. Technical Implementation on Solana

### Cryptographic Primitive

*   **Algorithm**: Pohlig-Hellman, which relies on modular exponentiation (`C^e mod p`).
*   **Security**: A 256-bit prime modulus `p` is sufficient. This provides security that is computationally infeasible to break within the short lifespan of a poker hand, while significantly reducing the on-chain compute cost compared to larger primes.
*   **Crate**: A Rust big integer library like `num-bigint` is required to perform the necessary `modpow` operations within the Anchor program.

### Compute Unit (CU) Management

*   The verification step is computationally expensive. By using a **progressive verification** model, the load is distributed across multiple transactions instead of a single, massive one at the end.
*   **Peak Cost**: The most expensive transactions will be the flop reveal (verifying 3 cards) and the final showdown (verifying up to 4 cards). These will likely require **400,000 - 500,000 CUs**.
*   **Requirement**: All client-side transactions that trigger a card reveal or showdown **must** include a `ComputeBudgetInstruction` to request a higher CU limit.

### Liveness and Security

*   **Griefing Prevention**: The protocol must include strict, on-chain timeouts for every player action (betting, and especially cryptographic actions like revealing community cards). The game state must track whose turn it is and when their deadline to act expires.
*   **Player Bonds**: To disincentivize stalling, each player is required to post a bond at the start of the game. If a player fails to act within a timeout period, their opponent can call a `claim_on_timeout` instruction, which awards them the pot and the stalling player's bond.
