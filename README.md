# zkPoker - permissionless, provably fair onchain poker on Solana

Featuring:
- **2-player heads-up poker** will expand to multiplayer in the future
- **Commutative Encryption** Pohlig-Hellman for provable fairness
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

zkPoker implements a trustless mental poker protocol using **Pohlig-Hellman commutative encryption**. The protocol ensures card privacy and fairness without requiring a trusted dealer.

### The Protocol

#### Phase 1: Game Setup & Deck Shuffling

1. **Key Generation**: Both players generate ephemeral keypairs for Pohlig-Hellman encryption
2. **Player 1 Setup**:
   - Shuffles a standard 52-card deck
   - Encrypts each card with their private key
   - Computes a **Merkle root** of the encrypted deck
   - Creates game on-chain with their public key and Merkle root
   - Passes encrypted deck to Player 2 off-chain

3. **Player 2 Join**:
   - Re-shuffles Player 1's encrypted deck
   - Applies second layer of encryption with their key
   - Selects 9 cards needed (4 pocket + 5 community)
   - Generates **Merkle proofs** for each card
   - Submits doubly-encrypted cards + proofs on-chain
   - Program verifies proofs against stored Merkle root

**Result**: 9 doubly-encrypted cards committed on-chain, provably from the original deck. Neither player knows plaintext values or can manipulate deck composition.

#### Phase 2: Gameplay

**Pocket Cards (Off-Chain)**:
- Player 2 sends Player 1's cards (decrypted once) off-chain
- Player 1's client **must verify** by re-encrypting and checking against on-chain values
- If verification fails, Player 1 can claim timeout
- Process mirrors for Player 2's cards

**Community Cards (On-Chain)**:
- Two-step progressive reveal for Flop, Turn, River:
  1. Player 1 submits decryption share
  2. Player 2 submits decryption share + plaintext
- Program **immediately verifies** by re-encrypting plaintext and comparing to stored encrypted version
- Distributes compute load across transactions
- Timeouts enforce liveness

**Betting**:
- Standard on-chain actions (Fold, Check, Call, Raise)
- Funds transferred to shared pot account

#### Phase 3: Showdown & Verification

1. **Card Reveal**: Players reveal plaintext pocket cards
2. **On-Chain Verification**: Program re-encrypts revealed cards with both keys and verifies against stored doubly-encrypted values
3. **Winner Determination**: Program evaluates verified hands using standard poker rankings and distributes pot

### Core Instructions

- **`create_game`** - P1 initializes game with public key and Merkle root
- **`join_game`** - P2 submits encrypted cards and Merkle proofs
- **`reveal_community_cards`** - Two-step card reveal with verification
- **`player_action`** - Betting actions (Fold, Check, Call, Raise)
- **`advance_street`** - Progress to next betting round
- **`resolve_hand`** - Showdown verification and winner determination
- **`claim_timeout`** - Win by timeout if opponent doesn't act

### Game State Progression

1. `WaitingForPlayer2` → P1 creates game
2. `PreFlopBetting` → P2 joins, blinds posted
3. `AwaitingFlopReveal` → Betting complete
4. `PostFlopBetting` → Flop revealed and verified
5. `AwaitingTurnReveal` → Betting complete
6. `PostTurnBetting` → Turn revealed and verified
7. `AwaitingRiverReveal` → Betting complete
8. `PostRiverBetting` → River revealed and verified
9. `Showdown` → Final betting complete
10. `Finished` → Winner determined, pot distributed

### Security Features

- **Provable Fairness**: Merkle proofs ensure deck integrity
- **Progressive Verification**: Cards verified immediately upon reveal (~400k CUs per verification)
- **Timeout Protection**: Player bonds forfeit if actions not taken in time
- **No Trusted Dealer**: Fully trustless cryptographic protocol
- **Client-Side Validation**: Critical verifications happen off-chain with on-chain enforcement

For complete details, see [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`MANIFESTO.md`](MANIFESTO.md).