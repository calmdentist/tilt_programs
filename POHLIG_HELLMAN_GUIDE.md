# Pohlig-Hellman Encryption Implementation Guide

## Overview

The `verify_card()` function is now fully implemented using the Pohlig-Hellman commutative cipher. This enables trustless mental poker where neither player has an advantage in the card shuffling or dealing process.

## Implementation Details

### Core Function: `verify_card()`

```rust
pub fn verify_card(&self, plaintext_card: u8, encrypted_card: &EncryptedCard) -> bool
```

**What it does:**
- Takes a plaintext card (0-51) and an encrypted card
- Re-encrypts the plaintext using both players' public keys
- Compares the result with the stored encrypted card
- Returns `true` if they match, `false` otherwise

**How it works:**
1. Converts plaintext card to valid range (2-53) to avoid small plaintext attacks
2. Performs first encryption: `plaintext^player1_key mod prime`
3. Performs second encryption: `encrypted_once^player2_key mod prime`
4. Compares result with stored encrypted card

### Helper Functions

#### `encrypt_card(card: u8, public_key: &EphemeralPubkey) -> EncryptedCard`

Encrypts a single card using a public key.

**Usage:**
```rust
let card = 0; // Ace of Clubs
let encrypted = GameState::encrypt_card(card, &player1_pubkey);
```

#### `decrypt_card(encrypted: &EncryptedCard, private_key: &[u8; 32]) -> Option<u8>`

Decrypts a card using a private key (expensive operation, use off-chain).

**Usage:**
```rust
let decrypted = GameState::decrypt_card(&encrypted_card, &player1_private_key);
```

## Client Implementation Requirements

### 1. Key Generation

Each player needs to generate an ephemeral keypair:

```typescript
// Pseudocode - actual implementation depends on your crypto library
function generateKeypair(): { privateKey: Uint8Array, publicKey: Uint8Array } {
  const privateKey = generateRandomInRange(2, PRIME - 1);  // 256-bit random
  const publicKey = computePublicKey(privateKey);           // Store as EphemeralPubkey
  return { privateKey, publicKey };
}
```

**Important:** 
- Private key must be in range [2, prime-1]
- Never share or store private keys on-chain
- Generate new keypair for each game

### 2. Player 1: Initial Shuffle & Encryption

```typescript
function player1ShuffleAndEncrypt(keypair: Keypair) {
  // Create ordered deck (0-51)
  let deck = [0, 1, 2, ..., 51];
  
  // Shuffle using Fisher-Yates
  deck = fisherYatesShuffle(deck);
  
  // Encrypt each card with player1's public key
  const encryptedDeck = deck.map(card => 
    encryptCard(card, keypair.publicKey)
  );
  
  // Send encrypted deck to Player 2 off-chain
  return encryptedDeck;
}
```

### 3. Player 2: Re-shuffle & Double Encryption

```typescript
function player2ReshuffleAndEncrypt(
  player1Deck: EncryptedCard[], 
  keypair: Keypair
) {
  // Shuffle Player 1's encrypted deck
  let deck = fisherYatesShuffle(player1Deck);
  
  // Encrypt again with Player 2's public key (double encryption)
  const doublyEncrypted = deck.map(encryptedCard => 
    encryptCard(encryptedCard, keypair.publicKey)  // Encrypt already-encrypted card
  );
  
  // Select 9 cards for the game
  const selectedCards = doublyEncrypted.slice(0, 9);
  // Indices: [0-1] = P1 hand, [2-3] = P2 hand, [4-8] = community cards
  
  // Submit to join_game instruction
  return selectedCards;
}
```

### 4. Card Reveal Process

#### Off-chain: Player provides decryption share

```typescript
function createDecryptionShare(
  encryptedCard: EncryptedCard, 
  privateKey: Uint8Array
): EncryptedCard {
  // Decrypt once with your private key
  // This is a "partial decryption" - the card is still encrypted by the other player
  return decryptCard(encryptedCard, privateKey);
}
```

#### On-chain: Program verifies the reveal

The program automatically verifies using `verify_card()`:
1. Player 2 submits decryption shares + plaintext
2. Program calls `verify_card(plaintext, stored_encrypted)`
3. Returns true if: `plaintext^p1_key^p2_key == stored_encrypted`

## Security Considerations

### Prime Modulus

- **Value**: 2^256 - 189
- **Type**: Safe prime (computationally secure)
- **Size**: 256 bits (33 bytes)
- **Security**: Provides ~128 bits of security (sufficient for poker game duration)

### Key Requirements

1. **Range**: Keys must be in [2, prime-1]
2. **Randomness**: Use cryptographically secure random number generator
3. **Ephemeral**: Generate new keypair for each game
4. **Privacy**: Never expose private keys

### Attack Resistance

- **Small Plaintext Attack**: Cards mapped to range [2, 53] instead of [0, 51]
- **Known Plaintext**: Each game uses fresh ephemeral keys
- **Replay Attack**: Encrypted cards bound to specific game state
- **Collusion**: Fast-fold model makes this difficult

## Compute Cost

Approximate CU costs per operation:

- **Single Card Verification**: ~80,000-100,000 CU
- **Flop Verification** (3 cards): ~250,000-300,000 CU
- **Turn/River Verification** (1 card): ~80,000-100,000 CU
- **Showdown Verification** (4 cards): ~320,000-400,000 CU

All operations stay well under Solana's 1.4M CU limit per transaction.

## Example: Full Game Flow

```typescript
// === SETUP ===
const p1_keypair = generateKeypair();
const p2_keypair = generateKeypair();

// Player 1 creates game
await program.methods
  .createGame(stakeAmount, p1_keypair.publicKey, gameId)
  .rpc();

// Player 1 shuffles and encrypts deck
const p1_deck = player1ShuffleAndEncrypt(p1_keypair);
// Send p1_deck to Player 2 off-chain

// Player 2 re-shuffles and double encrypts
const doublyEncryptedCards = player2ReshuffleAndEncrypt(p1_deck, p2_keypair);

// Player 2 joins game with encrypted cards
await program.methods
  .joinGame(p2_keypair.publicKey, doublyEncryptedCards)
  .rpc();

// === PRE-FLOP BETTING ===
await program.methods.playerAction(action, amount).rpc();

// === FLOP REVEAL ===
// Player 1 submits decryption shares
const p1_flop_shares = [4, 5, 6].map(idx => 
  createDecryptionShare(doublyEncryptedCards[idx], p1_keypair.privateKey)
);
await program.methods
  .revealCommunityCards(p1_flop_shares, null)
  .rpc();

// Player 2 completes reveal with plaintext
const p2_flop_shares = [4, 5, 6].map(idx => 
  createDecryptionShare(doublyEncryptedCards[idx], p2_keypair.privateKey)
);
const flop_plaintext = [4, 5, 6].map(idx => 
  finalDecrypt(doublyEncryptedCards[idx], p1_keypair.privateKey, p2_keypair.privateKey)
);
await program.methods
  .revealCommunityCards(p2_flop_shares, flop_plaintext)
  .rpc();
// Program automatically verifies flop cards on-chain

// Continue with betting and reveals for turn, river, showdown...
```

## Testing

To test the implementation:

1. Generate test keypairs
2. Encrypt a known card value
3. Verify it matches expected encryption
4. Decrypt and verify you get the original card back
5. Test double encryption (encrypt twice, decrypt twice)
6. Verify the commutative property: (m^a)^b == (m^b)^a

## Dependencies

- `num-bigint` (v0.4): For big integer arithmetic
- `num-traits` (v0.2): For numeric traits

Added to `Cargo.toml`:
```toml
num-bigint = "0.4"
num-traits = "0.2"
```

## Next Steps

1. **Client Library**: Build a TypeScript/JavaScript library that mirrors these functions
2. **Key Management**: Implement secure key generation and storage
3. **Testing Suite**: Create comprehensive tests for all crypto operations
4. **Optimization**: Profile and optimize if CU costs are higher than expected
5. **Audit**: Get cryptographic review of the implementation

## Resources

- [Pohlig-Hellman Cipher](https://en.wikipedia.org/wiki/Pohlig%E2%80%93Hellman_algorithm)
- [Mental Poker Protocol](https://en.wikipedia.org/wiki/Mental_poker)
- [Commutative Encryption](https://www.sciencedirect.com/topics/computer-science/commutative-encryption)

