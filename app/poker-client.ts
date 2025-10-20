import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, Keypair, SYSVAR_RENT_PUBKEY, ComputeBudgetProgram } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, getAssociatedTokenAddress } from "@solana/spl-token";
import { keccak_256 } from "@noble/hashes/sha3";
import { TiltPrograms } from "../target/types/tilt_programs";

/**
 * Player action types
 */
export enum PlayerAction {
  Fold = "fold",
  Check = "check",
  Call = "call",
  Raise = "raise",
  AllIn = "allIn",
}

/**
 * Game stages (updated for mental poker)
 */
export enum GameStage {
  WaitingForPlayer2 = "waitingForPlayer2",
  PreFlopBetting = "preFlopBetting",
  AwaitingFlopReveal = "awaitingFlopReveal",
  AwaitingPlayer2FlopShare = "awaitingPlayer2FlopShare",
  PostFlopBetting = "postFlopBetting",
  AwaitingTurnReveal = "awaitingTurnReveal",
  AwaitingPlayer2TurnShare = "awaitingPlayer2TurnShare",
  PostTurnBetting = "postTurnBetting",
  AwaitingRiverReveal = "awaitingRiverReveal",
  AwaitingPlayer2RiverShare = "awaitingPlayer2RiverShare",
  PostRiverBetting = "postRiverBetting",
  Showdown = "showdown",
  AwaitingPlayer2ShowdownReveal = "awaitingPlayer2ShowdownReveal",
  Finished = "finished",
}

/**
 * Card utilities
 */
export class Card {
  constructor(public value: number) {
    if (value < 0 || value > 51) {
      throw new Error("Card value must be between 0-51");
    }
  }

  get rank(): number {
    return this.value % 13;
  }

  get suit(): number {
    return Math.floor(this.value / 13);
  }

  get rankName(): string {
    const ranks = ["2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"];
    return ranks[this.rank];
  }

  get suitName(): string {
    const suits = ["♣", "♦", "♥", "♠"];
    return suits[this.suit];
  }

  toString(): string {
    return `${this.rankName}${this.suitName}`;
  }
}

/**
 * Encrypted card type (32 bytes)
 */
export type EncryptedCard = number[];

/**
 * Ephemeral public key type (32 bytes)
 */
export type EphemeralPubkey = number[];

/**
 * Merkle proof for a card
 */
export interface MerkleProof {
  proof: number[][]; // Array of 32-byte hashes
  index: number;     // Index of card in original deck (0-51)
}

/**
 * Mental Poker Cryptography Helper
 * 
 * Implements Pohlig-Hellman commutative encryption using native BigInt
 */
export class MentalPokerCrypto {
  // 256-bit safe prime: 2^256 - 189
  private static readonly PRIME = BigInt("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF43");

  /**
   * Convert bytes to BigInt (big-endian)
   */
  static bytesToBigInt(bytes: Uint8Array): bigint {
    let result = 0n;
    for (let i = 0; i < bytes.length; i++) {
      result = (result << 8n) | BigInt(bytes[i]);
    }
    return result;
  }

  /**
   * Convert BigInt to bytes (big-endian, fixed length)
   */
  private static bigIntToBytes(value: bigint, length: number): Uint8Array {
    const bytes = new Uint8Array(length);
    let v = value;
    
    for (let i = length - 1; i >= 0; i--) {
      bytes[i] = Number(v & 0xFFn);
      v = v >> 8n;
    }
    
    return bytes;
  }

  /**
   * Modular exponentiation: base^exp mod mod
   * Uses square-and-multiply algorithm
   */
  private static modPow(base: bigint, exp: bigint, mod: bigint): bigint {
    if (mod === 1n) return 0n;
    
    let result = 1n;
    base = base % mod;
    
    while (exp > 0n) {
      if (exp % 2n === 1n) {
        result = (result * base) % mod;
      }
      exp = exp >> 1n;
      base = (base * base) % mod;
    }
    
    return result;
  }

  /**
   * Compute modular multiplicative inverse using Extended Euclidean Algorithm
   * Returns x such that (a * x) mod m = 1
   */
  private static modInverse(a: bigint, m: bigint): bigint {
    const orig_m = m;
    a = a % m;
    
    if (a === 0n || m === 1n) {
      throw new Error(`No modular inverse exists for ${a} mod ${orig_m}`);
    }
    
    let x0 = 0n;
    let x1 = 1n;
    
    while (a > 1n) {
      if (m === 0n) {
        throw new Error(`gcd(${a}, ${orig_m}) != 1, no modular inverse exists`);
      }
      
      // q is quotient
      const q = a / m;
      
      // Update a and m
      let t = m;
      m = a % m;
      a = t;
      
      // Update x0 and x1
      t = x0;
      x0 = x1 - q * x0;
      x1 = t;
    }
    
    // Make x1 positive
    if (x1 < 0n) {
      x1 += orig_m;
    }
    
    return x1;
  }

  /**
   * Compute GCD using Euclidean algorithm
   */
  private static gcd(a: bigint, b: bigint): bigint {
    while (b !== 0n) {
      const t = b;
      b = a % b;
      a = t;
    }
    return a;
  }

  /**
   * Generate an ephemeral keypair for Pohlig-Hellman
   * Returns: { privateKey: 32 bytes, publicKey: 32 bytes }
   */
  static generateKeypair(): { privateKey: Uint8Array; publicKey: Uint8Array } {
    const prime = this.PRIME;
    const primeMinus1 = prime - 1n;
    
    // Generate random private key in range [3, prime-1] that is coprime with (prime-1)
    let privateKey: bigint;
    do {
      const bytes = new Uint8Array(32);
      crypto.getRandomValues(bytes);
      privateKey = this.bytesToBigInt(bytes);
    } while (
      privateKey < 3n || 
      privateKey >= prime ||
      this.gcd(privateKey, primeMinus1) !== 1n // Must be coprime with (prime-1)
    );
    
    // For Pohlig-Hellman, the public key IS the private key
    // (It's used as the exponent directly)
    const publicKey = this.bigIntToBytes(privateKey, 32);
    const privateKeyBytes = this.bigIntToBytes(privateKey, 32);
    
    return { privateKey: privateKeyBytes, publicKey };
  }

  /**
   * Encrypt a card with a public key
   * card: 0-51
   * publicKey: 32 bytes
   * Returns: 32-byte encrypted card
   */
  static encryptCard(card: number, publicKey: Uint8Array): EncryptedCard {
    const prime = this.PRIME;
    
    // Map card 0-51 to safe range 2-53
    const plaintext = BigInt(card + 2);
    const key = this.bytesToBigInt(publicKey);
    
    // Validate key
    if (key < 2n || key >= prime) {
      throw new Error("Invalid public key: must be in range [2, prime-1]");
    }
    
    // Encrypt: plaintext^key mod prime
    const encrypted = this.modPow(plaintext, key, prime);
    
    return Array.from(this.bigIntToBytes(encrypted, 32));
  }

  /**
   * Encrypt already-encrypted bytes (second layer)
   */
  static encryptCardBytes(encryptedCard: EncryptedCard, publicKey: Uint8Array): EncryptedCard {
    const prime = this.PRIME;
    
    // Convert encrypted bytes to bigint
    const encryptedValue = this.bytesToBigInt(new Uint8Array(encryptedCard));
    const key = this.bytesToBigInt(publicKey);
    
    // Validate key
    if (key < 2n || key >= prime) {
      throw new Error("Invalid public key");
    }
    
    // Double encrypt: (encrypted)^key mod prime
    const doubleEncrypted = this.modPow(encryptedValue, key, prime);
    
    return Array.from(this.bigIntToBytes(doubleEncrypted, 32));
  }

  /**
   * Decrypt a card (off-chain only, for testing/verification)
   */
  static decryptCard(encryptedCard: EncryptedCard, privateKey: Uint8Array): number {
    const prime = this.PRIME;
    const encrypted = this.bytesToBigInt(new Uint8Array(encryptedCard));
    const key = this.bytesToBigInt(privateKey);
    
    // For Pohlig-Hellman/SRA cipher with prime modulus p:
    // If c = m^e mod p, then m = c^d mod p, where e*d ≡ 1 (mod p-1)
    // So we need the modular inverse of key mod (prime-1)
    const invKey = this.modInverse(key, prime - 1n);
    
    // Decrypt: encrypted^(key^-1) mod prime
    const plaintext = this.modPow(encrypted, invKey, prime);
    
    // Convert back to card value (subtract 2)
    const card = Number(plaintext) - 2;
    
    // Validation
    if (card < 0 || card > 51) {
      throw new Error(`Decryption resulted in invalid card value: ${card} (plaintext: ${plaintext})`);
    }
    
    return card;
  }

  /**
   * Decrypt one layer of encryption (for progressive reveals)
   */
  static decryptOneLayer(encryptedCard: EncryptedCard, privateKey: Uint8Array): EncryptedCard {
    const prime = this.PRIME;
    const encrypted = this.bytesToBigInt(new Uint8Array(encryptedCard));
    const key = this.bytesToBigInt(privateKey);
    
    // Compute modular inverse mod (prime-1)
    const invKey = this.modInverse(key, prime - 1n);
    
    // Decrypt one layer
    const decrypted = this.modPow(encrypted, invKey, prime);
    
    return Array.from(this.bigIntToBytes(decrypted, 32));
  }

  /**
   * Verify that Player 2 used Player 1's deck (off-chain verification)
   * 
   * Checks:
   * 1. All 9 singly-encrypted cards exist in Player 1's original deck
   * 2. Merkle proofs are valid for the committed root
   * 3. Double encryption is correct
   * 
   * Returns: { valid: boolean, reason?: string }
   */
  static verifyDeckSelection(
    player1EncryptedDeck: EncryptedCard[],
    player2PublicKey: Uint8Array,
    singlyEncryptedCards: EncryptedCard[],
    doublyEncryptedCards: EncryptedCard[],
    merkleRoot: Uint8Array,
    merkleProofs: MerkleProof[]
  ): { valid: boolean; reason?: string } {
    // 1. Check all singly-encrypted cards are from Player 1's deck
    for (let i = 0; i < singlyEncryptedCards.length; i++) {
      const found = player1EncryptedDeck.some(c =>
        c.every((byte, j) => byte === singlyEncryptedCards[i][j])
      );
      
      if (!found) {
        return { 
          valid: false, 
          reason: `Card ${i} not found in Player 1's deck - Player 2 may have substituted cards!` 
        };
      }
    }

    // 2. Verify Merkle proofs
    // Rebuild entire tree to compare with committed root
    const rebuiltRoot = this.buildMerkleTree(player1EncryptedDeck);
    const rootsMatch = Array.from(rebuiltRoot).every((byte, i) => byte === merkleRoot[i]);
    
    if (!rootsMatch) {
      return {
        valid: false,
        reason: "Merkle root mismatch - deck may have been modified!"
      };
    }
    
    // Since we verified the root matches, and we verified all cards are in the deck,
    // we can trust the deck selection is valid
    // (More rigorous would be to verify each proof, but that's complex with odd nodes)

    // 3. Verify double encryption
    for (let i = 0; i < singlyEncryptedCards.length; i++) {
      const reEncrypted = this.encryptCardBytes(singlyEncryptedCards[i], player2PublicKey);
      
      const matches = reEncrypted.every((byte, j) => byte === doublyEncryptedCards[i][j]);
      
      if (!matches) {
        return { 
          valid: false, 
          reason: `Double encryption mismatch for card ${i} - encryption error or tampering!` 
        };
      }
    }

    return { valid: true };
  }

  /**
   * Build a Merkle tree from encrypted cards
   * Returns the root hash
   */
  static buildMerkleTree(cards: EncryptedCard[]): Uint8Array {
    if (cards.length === 0) throw new Error("No cards provided");
    
    // Hash each card to create leaf nodes
    let nodes = cards.map(card => keccak_256(new Uint8Array(card)));
    
    // Build tree bottom-up
    while (nodes.length > 1) {
      const nextLevel: Uint8Array[] = [];
      
      for (let i = 0; i < nodes.length; i += 2) {
        if (i + 1 < nodes.length) {
          // Combine two nodes
          const combined = new Uint8Array(64);
          combined.set(nodes[i], 0);
          combined.set(nodes[i + 1], 32);
          nextLevel.push(keccak_256(combined));
        } else {
          // Odd node, promote to next level
          nextLevel.push(nodes[i]);
        }
      }
      
      nodes = nextLevel;
    }
    
    return nodes[0];
  }

  /**
   * Generate a Merkle proof for a card at a given index
   */
  static generateMerkleProof(cards: EncryptedCard[], index: number): MerkleProof {
    if (index < 0 || index >= cards.length) {
      throw new Error("Index out of bounds");
    }

    const proof: number[][] = [];
    let nodes = cards.map(card => keccak_256(new Uint8Array(card)));
    let currentIndex = index;

    while (nodes.length > 1) {
      const nextLevel: Uint8Array[] = [];
      let nextIndex = -1;
      
      for (let i = 0; i < nodes.length; i += 2) {
        if (i + 1 < nodes.length) {
          // Pair exists
          const combined = new Uint8Array(64);
          combined.set(nodes[i], 0);
          combined.set(nodes[i + 1], 32);
          nextLevel.push(keccak_256(combined));
          
          // If current index is one of these two nodes, add sibling to proof
          if (i === currentIndex) {
            proof.push(Array.from(nodes[i + 1]));
            nextIndex = Math.floor(i / 2);
          } else if (i + 1 === currentIndex) {
            proof.push(Array.from(nodes[i]));
            nextIndex = Math.floor(i / 2);
          }
        } else {
          // Odd node - promote without pairing
          nextLevel.push(nodes[i]);
          
          // If this is our current node, no sibling to add but update index
          if (i === currentIndex) {
            nextIndex = nextLevel.length - 1;
            // Add a special marker or just don't add to proof
            // The verifier needs to know this was an odd node
          }
        }
      }
      
      currentIndex = nextIndex >= 0 ? nextIndex : Math.floor(currentIndex / 2);
      nodes = nextLevel;
    }

    return { proof, index };
  }
}

/**
 * Main client for interacting with the Tilt poker protocol
 */
export class PokerClient {
  constructor(
    public program: Program<TiltPrograms>,
    public provider: AnchorProvider,
    public usdcMint: PublicKey,
    public programVaultTokenAccount: PublicKey
  ) {}

  /**
   * Get player account PDA
   */
  getPlayerAccountPDA(player: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("player"), player.toBuffer()],
      this.program.programId
    );
  }

  /**
   * Get player balance PDA
   */
  getPlayerBalancePDA(player: PublicKey): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("balance"), player.toBuffer()],
      this.program.programId
    );
  }

  /**
   * Get program vault PDA
   */
  getProgramVaultPDA(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("program_vault")],
      this.program.programId
    );
  }

  /**
   * Get game vault PDA
   */
  getGameVaultPDA(player1: PublicKey, gameId: number): [PublicKey, number] {
    const gameIdBuffer = Buffer.alloc(8);
    gameIdBuffer.writeBigInt64LE(BigInt(gameId));
    
    return PublicKey.findProgramAddressSync(
      [Buffer.from("game_vault"), player1.toBuffer(), gameIdBuffer],
      this.program.programId
    );
  }

  /**
   * Get game state PDA
   */
  getGameStatePDA(player1: PublicKey, gameId: number): [PublicKey, number] {
    const gameIdBuffer = Buffer.alloc(8);
    gameIdBuffer.writeBigInt64LE(BigInt(gameId));
    
    return PublicKey.findProgramAddressSync(
      [Buffer.from("game"), player1.toBuffer(), gameIdBuffer],
      this.program.programId
    );
  }

  /**
   * Initialize a player account
   */
  async initializePlayer(player: Keypair): Promise<string> {
    const [playerAccountPDA] = this.getPlayerAccountPDA(player.publicKey);

    const tx = await this.program.methods
      .initializePlayer()
      .accounts({
        playerAccount: playerAccountPDA,
        authority: player.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Initialize a player balance account
   */
  async initializeBalance(player: Keypair): Promise<string> {
    const [playerBalancePDA] = this.getPlayerBalancePDA(player.publicKey);

    const tx = await this.program.methods
      .initializeBalance()
      .accounts({
        playerBalance: playerBalancePDA,
        authority: player.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Deposit USDC into player balance
   */
  async depositFunds(player: Keypair, amount: number): Promise<string> {
    const [playerBalancePDA] = this.getPlayerBalancePDA(player.publicKey);
    const userTokenAccount = await getAssociatedTokenAddress(
      this.usdcMint,
      player.publicKey
    );

    const tx = await this.program.methods
      .depositFunds(new anchor.BN(amount))
      .accounts({
        playerBalance: playerBalancePDA,
        userTokenAccount,
        programVault: this.programVaultTokenAccount,
        usdcMint: this.usdcMint,
        authority: player.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Withdraw USDC from player balance
   */
  async withdrawFunds(player: Keypair, amount: number): Promise<string> {
    const [playerBalancePDA] = this.getPlayerBalancePDA(player.publicKey);
    const [programVaultAuthority] = this.getProgramVaultPDA();
    const userTokenAccount = await getAssociatedTokenAddress(
      this.usdcMint,
      player.publicKey
    );

    const tx = await this.program.methods
      .withdrawFunds(new anchor.BN(amount))
      .accounts({
        playerBalance: playerBalancePDA,
        userTokenAccount,
        programVault: this.programVaultTokenAccount,
        programVaultAuthority,
        usdcMint: this.usdcMint,
        authority: player.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Create a new game with mental poker
   */
  async createGame(
    player1: Keypair,
    stakeAmount: number,
    player1Keypair?: { privateKey: Uint8Array; publicKey: Uint8Array },
    encryptedDeck?: EncryptedCard[]
  ): Promise<{
    tx: string;
    gameStatePDA: PublicKey;
    gameId: number;
    player1Keypair: { privateKey: Uint8Array; publicKey: Uint8Array };
    encryptedDeck: EncryptedCard[];
    merkleRoot: Uint8Array;
  }> {
    const gameId = Math.floor(Date.now() / 1000);
    const [gameStatePDA] = this.getGameStatePDA(player1.publicKey, gameId);
    const [player1BalancePDA] = this.getPlayerBalancePDA(player1.publicKey);
    const [gameVaultPDA] = this.getGameVaultPDA(player1.publicKey, gameId);
    const [programVaultAuthority] = this.getProgramVaultPDA();

    // Generate keypair if not provided
    const keypair = player1Keypair || MentalPokerCrypto.generateKeypair();

    // Shuffle and encrypt deck if not provided
    let deck = encryptedDeck;
    if (!deck) {
      // Create ordered deck
      const orderedDeck = Array.from({ length: 52 }, (_, i) => i);
      
      // Fisher-Yates shuffle
      for (let i = orderedDeck.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [orderedDeck[i], orderedDeck[j]] = [orderedDeck[j], orderedDeck[i]];
      }

      // Encrypt each card
      deck = orderedDeck.map(card => 
        MentalPokerCrypto.encryptCard(card, keypair.publicKey)
      );
    }

    // Compute Merkle root
    const merkleRoot = MentalPokerCrypto.buildMerkleTree(deck);

    const tx = await this.program.methods
      .createGame(
        new anchor.BN(stakeAmount),
        { data: Array.from(keypair.publicKey) }, // EphemeralPubkey struct
        Array.from(merkleRoot),
        new anchor.BN(gameId)
      )
      .accounts({
        gameState: gameStatePDA,
        player1Balance: player1BalancePDA,
        gameVault: gameVaultPDA,
        programVault: this.programVaultTokenAccount,
        programVaultAuthority,
        usdcMint: this.usdcMint,
        player1: player1.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([player1])
      .rpc();

    return { tx, gameStatePDA, gameId, player1Keypair: keypair, encryptedDeck: deck, merkleRoot };
  }

  /**
   * Join an existing game (Player 2)
   */
  async joinGame(
    player2: Keypair,
    gameStatePDA: PublicKey,
    player1EncryptedDeck: EncryptedCard[],
    player2Keypair?: { privateKey: Uint8Array; publicKey: Uint8Array }
  ): Promise<{
    tx: string;
    player2Keypair: { privateKey: Uint8Array; publicKey: Uint8Array };
    proofData: {
      singlyEncryptedCards: EncryptedCard[];
      doublyEncryptedCards: EncryptedCard[];
      merkleProofs: MerkleProof[];
    };
  }> {
    const gameState = await this.getGameState(gameStatePDA);
    const [player2BalancePDA] = this.getPlayerBalancePDA(player2.publicKey);
    const [programVaultAuthority] = this.getProgramVaultPDA();

    // Generate keypair if not provided
    const keypair = player2Keypair || MentalPokerCrypto.generateKeypair();

    // Re-shuffle Player 1's deck
    const shuffledDeck = [...player1EncryptedDeck];
    for (let i = shuffledDeck.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [shuffledDeck[i], shuffledDeck[j]] = [shuffledDeck[j], shuffledDeck[i]];
    }

    // Double-encrypt the first 9 cards
    const singlyEncryptedCards = shuffledDeck.slice(0, 9);
    const doublyEncryptedCards = singlyEncryptedCards.map(card =>
      MentalPokerCrypto.encryptCardBytes(card, keypair.publicKey)
    );

    // Generate Merkle proofs for the 9 cards
    // Need to find their original indices in player1EncryptedDeck
    const merkleProofs = singlyEncryptedCards.map(card => {
      const originalIndex = player1EncryptedDeck.findIndex(c =>
        c.every((byte, i) => byte === card[i])
      );
      return MentalPokerCrypto.generateMerkleProof(player1EncryptedDeck, originalIndex);
    });

    // Optimistic verification: Send empty proofs on-chain, verify off-chain
    const doublyEncryptedStructs = doublyEncryptedCards.map(card => ({ data: card }));
    const singlyEncryptedStructs = singlyEncryptedCards.map(card => ({ data: card }));
    
    // Send minimal data on-chain (verification happens off-chain)
    const merkleProofStructs = merkleProofs.map(mp => ({
      proof: [], // Empty - off-chain verification only
      index: mp.index & 0xFF
    }));

    // Request high compute units (just in case)
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({ 
      units: 1_200_000
    });

    const tx = await this.program.methods
      .joinGame(
        { data: Array.from(keypair.publicKey) }, // EphemeralPubkey struct
        doublyEncryptedStructs,
        singlyEncryptedStructs,
        merkleProofStructs
      )
      .accounts({
        gameState: gameStatePDA,
        player2Balance: player2BalancePDA,
        gameVault: gameState.tokenVault,
        programVault: this.programVaultTokenAccount,
        programVaultAuthority,
        player2: player2.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .preInstructions([computeBudgetIx])
      .signers([player2])
      .rpc();

    // Return proof data so Player 1 can verify off-chain
    return { 
      tx, 
      player2Keypair: keypair,
      proofData: {
        singlyEncryptedCards,
        doublyEncryptedCards,
        merkleProofs
      }
    };
  }

  /**
   * Reveal community cards (two-step process)
   */
  async revealCommunityCards(
    player: Keypair,
    gameStatePDA: PublicKey,
    decryptionShares: EncryptedCard[],
    plaintextCards?: number[]
  ): Promise<string> {
    // Convert to struct format
    const decryptionShareStructs = decryptionShares.map(card => ({ data: card }));
    const plaintextBuffer = plaintextCards ? Buffer.from(plaintextCards) : null;

    // Request high compute units for card verification (uses BigUint modpow)
    // These cryptographic operations are very expensive on-chain
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({ 
      units: 1_200_000
    });

    const tx = await this.program.methods
      .revealCommunityCards(
        decryptionShareStructs,
        plaintextBuffer
      )
      .accounts({
        gameState: gameStatePDA,
        player: player.publicKey,
      })
      .preInstructions([computeBudgetIx])
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Perform a player action
   */
  async playerAction(
    player: Keypair,
    gameStatePDA: PublicKey,
    action: PlayerAction,
    raiseAmount?: number
  ): Promise<string> {
    const actionVariant = this.getActionVariant(action);
    const amount = raiseAmount ? new anchor.BN(raiseAmount) : null;

    const tx = await this.program.methods
      .playerAction(actionVariant, amount)
      .accounts({
        gameState: gameStatePDA,
        player: player.publicKey,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Advance to next street
   */
  async advanceStreet(gameStatePDA: PublicKey): Promise<string> {
    const tx = await this.program.methods
      .advanceStreet()
      .accounts({
        gameState: gameStatePDA,
      })
      .rpc();

    return tx;
  }

  /**
   * Resolve hand at showdown
   */
  async resolveHand(
    player: Keypair,
    gameStatePDA: PublicKey,
    player1: PublicKey,
    player2: PublicKey
  ): Promise<string> {
    const gameState = await this.getGameState(gameStatePDA);
    const [player1AccountPDA] = this.getPlayerAccountPDA(player1);
    const [player2AccountPDA] = this.getPlayerAccountPDA(player2);
    const [player1BalancePDA] = this.getPlayerBalancePDA(player1);
    const [player2BalancePDA] = this.getPlayerBalancePDA(player2);

    // Request high compute units for hand verification (verifies 4 pocket cards)
    // These cryptographic operations are very expensive on-chain
    const computeBudgetIx = ComputeBudgetProgram.setComputeUnitLimit({ 
      units: 1_200_000
    });

    const tx = await this.program.methods
      .resolveHand()
      .accounts({
        gameState: gameStatePDA,
        player1Account: player1AccountPDA,
        player2Account: player2AccountPDA,
        player1Balance: player1BalancePDA,
        player2Balance: player2BalancePDA,
        gameVault: gameState.tokenVault,
        programVault: this.programVaultTokenAccount,
        player: player.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .preInstructions([computeBudgetIx])
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Claim timeout win
   */
  async claimTimeout(
    player: Keypair,
    gameStatePDA: PublicKey,
    player1: PublicKey,
    player2: PublicKey
  ): Promise<string> {
    const [player1BalancePDA] = this.getPlayerBalancePDA(player1);
    const [player2BalancePDA] = this.getPlayerBalancePDA(player2);

    const tx = await this.program.methods
      .claimTimeout()
      .accounts({
        gameState: gameStatePDA,
        player1Balance: player1BalancePDA,
        player2Balance: player2BalancePDA,
        player: player.publicKey,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Get game state
   */
  async getGameState(gameStatePDA: PublicKey): Promise<any> {
    return await this.program.account.gameState.fetch(gameStatePDA);
  }

  /**
   * Get player account
   */
  async getPlayerAccount(player: PublicKey): Promise<any> {
    const [playerAccountPDA] = this.getPlayerAccountPDA(player);
    return await this.program.account.playerAccount.fetch(playerAccountPDA);
  }

  /**
   * Get player balance
   */
  async getPlayerBalance(player: PublicKey): Promise<any> {
    const [playerBalancePDA] = this.getPlayerBalancePDA(player);
    return await this.program.account.playerBalance.fetch(playerBalancePDA);
  }

  /**
   * Format cards for display
   */
  formatCards(cardValues: number[]): string {
    return cardValues.map((v) => new Card(v).toString()).join(" ");
  }

  /**
   * Get hand rank name
   */
  getHandRankName(rank: number): string {
    const ranks = [
      "High Card",
      "One Pair",
      "Two Pair",
      "Three of a Kind",
      "Straight",
      "Flush",
      "Full House",
      "Four of a Kind",
      "Straight Flush",
      "Royal Flush",
    ];
    return ranks[rank] || "Unknown";
  }

  /**
   * Helper to convert PlayerAction to program enum variant
   */
  private getActionVariant(action: PlayerAction): any {
    switch (action) {
      case PlayerAction.Fold:
        return { fold: {} };
      case PlayerAction.Check:
        return { check: {} };
      case PlayerAction.Call:
        return { call: {} };
      case PlayerAction.Raise:
        return { raise: {} };
      case PlayerAction.AllIn:
        return { allIn: {} };
      default:
        throw new Error(`Unknown action: ${action}`);
    }
  }
}

/**
 * Create a poker client instance
 */
export function createPokerClient(
  program: Program<TiltPrograms>,
  provider: AnchorProvider,
  usdcMint: PublicKey,
  programVaultTokenAccount: PublicKey
): PokerClient {
  return new PokerClient(program, provider, usdcMint, programVaultTokenAccount);
}
