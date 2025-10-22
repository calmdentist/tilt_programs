# zkPoker - permissionless, provably fair onchain poker on Solana

Featuring:
- **2-player heads-up poker** will expand to multiplayer in the future
- **Commutative Encryption** Paillier's cryptosystem for provable fairness
- **Complete poker hand evaluation** with standard Texas Hold'em rules
- **Time-limited actions** with timeout protection
- **Bespoke architecture** only 1 txn to create/join game and each betting round

## 🚀 Quick Setup

### Prerequisites
- **Bun** - JavaScript runtime and package manager
- **Anchor CLI** - Version ^0.28
- **Solana CLI** - Version ^1.18

### Installation & Demo
```bash
# Make the setup script executable
chmod +x ./setup.sh

# Run the complete setup and demo
./setup.sh
```

This will:
1. Install dependencies
2. Build the program
3. Start a local Solana validator
4. Deploy the program
5. Run a full game demonstration

## 🎯 Architecture Overview

zkPoker implements a trustless mental poker protocol built on two core principles: **optimistic execution** and **cryptographic guarantees**. Most actions are assumed to be valid to save on gas, with on-chain ZK-SNARK verification used as an impartial judge to resolve disputes. However, the fairness of the deck itself is guaranteed by a mandatory, on-chain verified ZKP at the start of every hand.

For complete details, see [`ARCHITECTURE.md`](ARCHITECTURE.md).

### Core Cryptography

-   **Commutative Encryption (Paillier)**: We use the Paillier cryptosystem because it is **probabilistic**. This means encrypting the same card multiple times produces different ciphertexts, a critical feature that prevents brute-force attacks.
-   **Zero-Knowledge Proofs (Groth16 ZK-SNARKs)**: We use Groth16 proofs because they are extremely small and efficient to verify on-chain. This makes our dispute resolution mechanism fast and affordable.

### The Protocol in Brief

1.  **Match Setup**: Two players join a game, each depositing funds and committing their Paillier public key to the on-chain `Game` account.

2.  **The Secure Shuffle**:
    -   **Player A** (creator) creates a 52-card deck, encrypts it, and submits its **merkle root** to the contract. This is accompanied by a mandatory `ProveCorrectDeckCreation` ZKP, which is **verified on-chain immediately** to guarantee the deck is fair.
    -   **Player B** (dealer) receives the encrypted deck off-chain, re-shuffles and re-encrypts it, and submits the **merkle root** of this final deck to the contract, along with an optimistic `ProveCorrectReshuffle` ZKP.

3.  **Optimistic Gameplay**: Players take turns betting. When a card is revealed, the player provides the decrypted data and an optimistic `ProveCorrectDecryption` ZKP. Opponents' clients verify these proofs off-chain.

4.  **Dispute & Resolution**: If a client detects an invalid proof, the player calls `claim_timeout`. This forces an on-chain verification of the disputed ZKP. If it fails, the cheater forfeits the pot.

### Core Instructions

-   **Match Setup**: `create_game`, `join_game`
-   **Hand Lifecycle**: `create_hand`, `join_hand`
-   **Gameplay**: `player_action`
-   **Showdown & Resolution**: `showdown`, `resolve_hand`
-   **Disputes & Match End**: `claim_timeout`, `leave_game`

### Security Features

-   **Provable Deck Fairness**: Mandatory ZKP on deck creation prevents stacked/duplicate cards.
-   **Trustless Dispute Resolution**: Optimistic ZKPs allow any player to police cheating.
-   **No Trusted Dealer**: The protocol is fully trustless and managed by the two players.
-   **Client-Side Validation**: Ensures immediate detection of invalid actions off-chain.
-   **Timeout Protection**: Liveness is enforced by on-chain timeouts for all actions.