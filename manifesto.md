# zkPoker: Permissionless, Provably Fair Poker on Solana

## 1. Overview

zkPoker is a protocol for a secure, trustless, two-player (heads-up) poker game on the Solana blockchain. Its purpose is to create an on-chain poker experience where fairness is not assumed but cryptographically guaranteed, removing the need for any trusted third party or centralized server.

The design is built on three core principles:

1.  **Optimistic Execution**: To maximize gas efficiency, the on-chain program does not verify most cryptographic proofs during gameplay. It assumes players are acting honestly until a dispute is formally raised.
2.  **Cryptographic Guarantees**: The fairness of the deck is **not** optimistic. It is guaranteed at the start of every hand using a mandatory, on-chain verified Zero-Knowledge Proof, preventing stacked or duplicate card attacks from the outset.
3.  **Client-Side Computation**: All heavy cryptographic operations (encryption, proof generation) are handled by the players' local clients. The blockchain only acts as a state machine and an impartial judge for disputes.

## 2. Core Cryptography

The protocol's security relies on two key cryptographic primitives:

### Commutative Encryption: Paillier Cryptosystem

To achieve a secure, multi-party shuffle, we use the Paillier cryptosystem. Unlike deterministic schemes, Paillier is **probabilistic**, meaning the same card encrypted twice will produce a different ciphertext each time. This is a critical feature that prevents players from brute-force guessing cards by re-encrypting all 52 possibilities.

### Zero-Knowledge Proofs: ZK-SNARKs (Groth16)

To prove actions were performed correctly without revealing private information (like a player's private key), we use ZK-SNARKs. We specifically use the Groth16 proving system because it produces proofs that are extremely small and cheap to verify on-chain, making the dispute resolution process highly efficient.

The key proofs are:
-   **`ProveCorrectDeckCreation` (Mandatory)**: Guarantees the initial deck is fair.
-   **`ProveCorrectReshuffle` & `ProveCorrectDecryption` (Optimistic)**: Used to resolve disputes during gameplay.

## 3. The Protocol in Brief

1.  **Match Setup**: Two players join a game, each posting their Paillier public key and funding their stacks. These keys are persistent for the entire match.
2.  **The Secure Shuffle**: For each hand, Player A creates and commits to a 52-card deck, proven fair with an on-chain verified `ProveCorrectDeckCreation` ZKP. Player B then re-shuffles and re-encrypts this deck, submitting an optimistic `ProveCorrectReshuffle` ZKP.
3.  **Optimistic Gameplay**: Players take turns betting and revealing cards. All card reveals are accompanied by an optimistic `ProveCorrectDecryption` ZKP. Each player's client verifies these proofs off-chain.
4.  **Dispute Resolution**: If a client detects an invalid proof, the player can call `claim_timeout` on-chain. This forces the accused player's ZKP to be verified by the program. If the proof is invalid, the cheater forfeits the pot. This makes the game self-policing and trustless.
