# Tilt - Permissionless, provably fair onchain poker on Solana

A fully on-chain, permissionless, provably fair poker protocol on Solana featuring:
- **2-player heads-up poker** will expand to multiplayer in the future
- **Commutative Encryption** for provable fairness
- **Complete poker hand evaluation** with standard Texas Hold'em rules
- **Time-limited actions** with timeout protection
- **Persistent player statistics**

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

### Game Flow

1. **Game Creation** - Player 1 creates a game with:
   - Stake amount
   - Commitment hash (keccak256 of secret)

2. **Player Join** - Player 2 joins with:
   - Their own commitment hash

3. **Secret Reveal** - Both players reveal their secrets:
   - Secrets are verified against commitments
   - Combined seed generates shuffled deck
   - Blinds are automatically posted

4. **Deal Cards** - Initial pocket cards dealt

5. **Betting Rounds**:
   - **Pre-flop** - Betting with hole cards only
   - **Flop** - 3 community cards dealt, betting round
   - **Turn** - 4th community card dealt, betting round
   - **River** - 5th community card dealt, betting round

6. **Showdown** - Best 5-card hands evaluated, winner determined

7. **Resolution** - Pot distributed, player stats updated

## 🔐 Provable Fairness

The protocol uses a **commit-reveal scheme** to ensure neither player can manipulate the deck:

1. Both players commit to a random 32-byte secret (by submitting its hash)
2. After both commit, they reveal their secrets
3. Secrets are XORed and hashed to create the deck seed
4. Deck is shuffled deterministically using Fisher-Yates algorithm

This ensures:
- Neither player knows the deck order before committing
- The deck is verifiable by combining both revealed secrets
- No third party or oracle is needed

## 📝 Program Instructions

### `initialize_player`
Creates a persistent player account to track stats across games.

**Accounts:**
- `player_account` - PDA derived from player's pubkey
- `authority` - Signer and payer

### `create_game`
Player 1 creates a new game.

**Parameters:**
- `stake_amount: u64` - Amount staked by each player
- `commitment: [u8; 32]` - Keccak256 hash of player's secret

**Accounts:**
- `game_state` - PDA for game data
- `player1` - Signer and payer

### `join_game`
Player 2 joins an existing game.

**Parameters:**
- `commitment: [u8; 32]` - Keccak256 hash of player's secret

**Accounts:**
- `game_state` - Game to join
- `player2` - Signer

### `reveal_secret`
Players reveal their secrets to generate the deck.

**Parameters:**
- `secret: [u8; 32]` - Original secret that was hashed

**Accounts:**
- `game_state` - Game state
- `player` - Signer (either player)

### `deal_initial`
Deals 2 pocket cards to each player (can be called by anyone).

**Accounts:**
- `game_state` - Game state

### `player_action`
Player performs an action during betting round.

**Parameters:**
- `action: PlayerActionType` - Fold, Check, Call, Raise, or AllIn
- `raise_amount: Option<u64>` - Amount to raise (required for Raise)

**Accounts:**
- `game_state` - Game state
- `player` - Signer (must be current player)

### `advance_street`
Advances to next street after betting round completes (can be called by anyone).

**Accounts:**
- `game_state` - Game state

### `resolve_game`
Evaluates hands and determines winner at showdown.

**Accounts:**
- `game_state` - Game state
- `player1_account` - Player 1's stats account
- `player2_account` - Player 2's stats account

### `claim_timeout`
Claim win if opponent doesn't act within time limit.

**Accounts:**
- `game_state` - Game state
- `player` - Signer (non-timeout player)

## 🃏 Card Representation

Cards are represented as `u8` values (0-51):
- **0-12**: Clubs (2-A)
- **13-25**: Diamonds (2-A)
- **26-38**: Hearts (2-A)
- **39-51**: Spades (2-A)

Within each suit:
- 0 = 2, 1 = 3, ..., 8 = 10, 9 = J, 10 = Q, 11 = K, 12 = A

## 🎲 Hand Rankings

Standard poker hand rankings (highest to lowest):
1. Royal Flush
2. Straight Flush
3. Four of a Kind
4. Full House
5. Flush
6. Straight
7. Three of a Kind
8. Two Pair
9. One Pair
10. High Card

## 🔧 Client Usage Example

```typescript
import * as anchor from "@coral-xyz/anchor";
import { keccak_256 } from "@noble/hashes/sha3";

// Generate secret and commitment
const secret = new Uint8Array(32);
crypto.getRandomValues(secret);
const commitment = keccak_256(secret);

// Create game
await program.methods
  .createGame(
    new anchor.BN(1000000), // 1 SOL stake
    Array.from(commitment)
  )
  .accounts({
    gameState: gamePda,
    player1: player1.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([player1])
  .rpc();

// Join game (as player 2)
await program.methods
  .joinGame(Array.from(commitment2))
  .accounts({
    gameState: gamePda,
    player2: player2.publicKey,
  })
  .signers([player2])
  .rpc();

// Reveal secrets
await program.methods
  .revealSecret(Array.from(secret))
  .accounts({
    gameState: gamePda,
    player: player1.publicKey,
  })
  .signers([player1])
  .rpc();

// Deal cards
await program.methods
  .dealInitial()
  .accounts({
    gameState: gamePda,
  })
  .rpc();

// Player action
await program.methods
  .playerAction(
    { raise: {} }, // Action type
    new anchor.BN(2000000) // Raise amount
  )
  .accounts({
    gameState: gamePda,
    player: currentPlayer.publicKey,
  })
  .signers([currentPlayer])
  .rpc();

// Advance to next street
await program.methods
  .advanceStreet()
  .accounts({
    gameState: gamePda,
  })
  .rpc();

// Resolve game at showdown
await program.methods
  .resolveGame()
  .accounts({
    gameState: gamePda,
    player1Account: player1AccountPda,
    player2Account: player2AccountPda,
  })
  .rpc();
```

## 🏗️ Development

### Build
```bash
anchor build
```

### Test
```bash
anchor test
```

### Deploy
```bash
anchor deploy
```

## 🔒 Security Considerations

1. **Randomness**: Uses commit-reveal scheme - both players contribute entropy
2. **Timeouts**: 60-second action timeout prevents griefing
3. **Atomicity**: All game state changes are atomic on-chain
4. **Verification**: Entire game state is on-chain and verifiable
5. **No Oracle**: No external randomness source needed

## 🚀 Future Enhancements

- [ ] Multi-table tournaments
- [ ] Rake/fee mechanism
- [ ] Player reputation system
- [ ] Hand history export
- [ ] Side pots for all-in scenarios
- [ ] Table chat/emotes
- [ ] Automated matchmaking contract
- [ ] Progressive jackpots

## 📄 License

MIT

## 🤝 Contributing

Contributions welcome! Please open an issue or PR.

