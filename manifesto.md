# zkPoker: Permissionless, provably fair poker on Solana

## 1. Overview

This document outlines a cryptographic protocol for implementing a secure, trustless, two-player (heads-up) poker game on the Solana blockchain. The core challenge is to manage the deck and player hands without a central, trusted dealer, ensuring that no player has an unfair advantage or access to hidden information. Permissionless, provably fair, unstoppable poker.

The protocol uses a heads-up (2 player), fast-fold (pair with new player after each hand) model, which mitigates colluding and the use of solvers to a large degree, if not entirely.

This is achieved using a commutative encryption scheme - Polhig-Hellman - and a bespoke architecture with the property that players only place 1 transaction to create/join a game, and 1 transaction per action (call, bet, fold, etc). CUs are optimized so that the most intensive instruction consumes less than 400,000 CUs.

## 2. The Protocol

The protocol is divided into three main phases: Setup, Gameplay, and Verification.

### Phase 1: Game Setup & Shuffling

The goal of this phase is to create a shuffled, encrypted deck that is committed to the on-chain game state, preventing any party from manipulating the deck.

1.  **Key Generation**: Both players (Player 1 and Player 2) generate ephemeral keypairs for a **commutative encryption scheme** (e.g., Pohlig-Hellman).
2.  **Player 1's Shuffle, Encrypt, and Commit**: Player 1 takes a canonical 52-card deck, shuffles it locally, and encrypts each card with their ephemeral public key. They then compute a **Merkle root** of this 52-card singly-encrypted deck. To create the game, Player 1 submits their public key and this Merkle root to the program. The full encrypted deck is then passed to Player 2 off-chain.
3.  **Player 2's Shuffle, Encrypt, and Prove**: Player 2 takes the singly-encrypted deck, shuffles it again locally, and applies a second layer of encryption using their own key. From this final deck, Player 2 selects the 9 cards necessary for the hand (2 pocket cards for each player, 5 community cards). To join the game, Player 2 must submit these 9 doubly-encrypted cards to the program, along with **Merkle proofs** for each of the 9 cards, proving they originated from the deck Player 1 committed to. The program verifies these proofs against the stored Merkle root before allowing the game to start.

At the end of this phase, the 9 cards for the hand are provably selected from the original shuffled deck and are committed to the chain. No single party knows their plaintext values, and neither party could have manipulated the deck composition.

### Phase 2: Gameplay

During gameplay, cards are revealed to individual players (pocket cards) or both players (community cards) without exposing the rest of the deck.

1.  **Dealing Pocket Cards**: To deal a card to Player 1, Player 2 provides the singly-decrypted card to Player 1 (decrypted with Player 2's key). This happens via off-chain communication.
    *   **Client-Side Verification**: Before Player 1 accepts and decrypts the card, their client **must** perform a crucial verification step. The client re-encrypts the received card with Player 2's public key and confirms that the result matches the original doubly-encrypted card stored on-chain. This proves Player 2 acted honestly.
    *   If the verification fails, Player 2 is considered to be uncooperative, and Player 1 can claim victory via the timeout mechanism.
    *   This entire process is mirrored for Player 2's cards. The program only needs to know which encrypted cards were assigned to which player.
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
