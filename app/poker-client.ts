import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, Keypair } from "@solana/web3.js";
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
 * Game stages
 */
export enum GameStage {
  WaitingForPlayers = "waitingForPlayers",
  WaitingForCommitments = "waitingForCommitments",
  WaitingForReveals = "waitingForReveals",
  PreFlop = "preFlop",
  Flop = "flop",
  Turn = "turn",
  River = "river",
  Showdown = "showdown",
  Completed = "completed",
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
 * Main client for interacting with the Tilt poker protocol
 */
export class PokerClient {
  constructor(
    public program: Program<TiltPrograms>,
    public provider: AnchorProvider
  ) {}

  /**
   * Generate a random secret and its commitment
   */
  generateCommitment(): { secret: Uint8Array; commitment: Uint8Array } {
    const secret = new Uint8Array(32);
    crypto.getRandomValues(secret);
    const commitment = keccak_256(secret);
    return { secret, commitment };
  }

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
   * Get game state PDA
   */
  getGameStatePDA(player1: PublicKey, timestamp: number): [PublicKey, number] {
    const timestampBuffer = Buffer.alloc(8);
    timestampBuffer.writeBigInt64LE(BigInt(timestamp));
    
    return PublicKey.findProgramAddressSync(
      [Buffer.from("game"), player1.toBuffer(), timestampBuffer],
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
   * Create a new game
   */
  async createGame(
    player1: Keypair,
    stakeAmount: number,
    commitment?: Uint8Array
  ): Promise<{ tx: string; gameStatePDA: PublicKey; secret: Uint8Array }> {
    const timestamp = Math.floor(Date.now() / 1000);
    const [gameStatePDA] = this.getGameStatePDA(player1.publicKey, timestamp);

    const { secret, commitment: comm } = commitment
      ? { secret: new Uint8Array(), commitment }
      : this.generateCommitment();

    const tx = await this.program.methods
      .createGame(new anchor.BN(stakeAmount), Array.from(commitment || comm))
      .accounts({
        gameState: gameStatePDA,
        player1: player1.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([player1])
      .rpc();

    return { tx, gameStatePDA, secret };
  }

  /**
   * Join an existing game
   */
  async joinGame(
    player2: Keypair,
    gameStatePDA: PublicKey,
    commitment?: Uint8Array
  ): Promise<{ tx: string; secret: Uint8Array }> {
    const { secret, commitment: comm } = commitment
      ? { secret: new Uint8Array(), commitment }
      : this.generateCommitment();

    const tx = await this.program.methods
      .joinGame(Array.from(commitment || comm))
      .accounts({
        gameState: gameStatePDA,
        player2: player2.publicKey,
      })
      .signers([player2])
      .rpc();

    return { tx, secret };
  }

  /**
   * Reveal secret
   */
  async revealSecret(
    player: Keypair,
    gameStatePDA: PublicKey,
    secret: Uint8Array
  ): Promise<string> {
    const tx = await this.program.methods
      .revealSecret(Array.from(secret))
      .accounts({
        gameState: gameStatePDA,
        player: player.publicKey,
      })
      .signers([player])
      .rpc();

    return tx;
  }

  /**
   * Deal initial cards
   */
  async dealInitial(gameStatePDA: PublicKey): Promise<string> {
    const tx = await this.program.methods
      .dealInitial()
      .accounts({
        gameState: gameStatePDA,
      })
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
   * Resolve game
   */
  async resolveGame(
    gameStatePDA: PublicKey,
    player1: PublicKey,
    player2: PublicKey
  ): Promise<string> {
    const [player1AccountPDA] = this.getPlayerAccountPDA(player1);
    const [player2AccountPDA] = this.getPlayerAccountPDA(player2);

    const tx = await this.program.methods
      .resolveGame()
      .accounts({
        gameState: gameStatePDA,
        player1Account: player1AccountPDA,
        player2Account: player2AccountPDA,
      })
      .rpc();

    return tx;
  }

  /**
   * Claim timeout win
   */
  async claimTimeout(player: Keypair, gameStatePDA: PublicKey): Promise<string> {
    const tx = await this.program.methods
      .claimTimeout()
      .accounts({
        gameState: gameStatePDA,
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
  provider: AnchorProvider
): PokerClient {
  return new PokerClient(program, provider);
}

/**
 * Complete game flow helper
 */
export class GameFlow {
  constructor(private client: PokerClient) {}

  /**
   * Run a complete game flow
   */
  async runCompleteGame(
    player1: Keypair,
    player2: Keypair,
    stakeAmount: number,
    onProgress?: (stage: string, data?: any) => void
  ): Promise<void> {
    try {
      // Initialize players if needed
      onProgress?.("Initializing players...");
      try {
        await this.client.initializePlayer(player1);
      } catch (e) {
        // Player already initialized
      }
      try {
        await this.client.initializePlayer(player2);
      } catch (e) {
        // Player already initialized
      }

      // Create game
      onProgress?.("Creating game...");
      const { gameStatePDA, secret: secret1 } = await this.client.createGame(
        player1,
        stakeAmount
      );
      onProgress?.("Game created", { gameStatePDA: gameStatePDA.toString() });

      // Join game
      onProgress?.("Player 2 joining...");
      const { secret: secret2 } = await this.client.joinGame(player2, gameStatePDA);
      onProgress?.("Player 2 joined");

      // Reveal secrets
      onProgress?.("Revealing secrets...");
      await this.client.revealSecret(player1, gameStatePDA, secret1);
      await this.client.revealSecret(player2, gameStatePDA, secret2);
      onProgress?.("Secrets revealed, deck shuffled");

      // Deal cards
      onProgress?.("Dealing cards...");
      await this.client.dealInitial(gameStatePDA);
      
      const gameState = await this.client.getGameState(gameStatePDA);
      onProgress?.("Cards dealt", {
        player1Hand: this.client.formatCards(gameState.player1Hand),
        player2Hand: this.client.formatCards(gameState.player2Hand),
      });

      // Now the game is ready for betting rounds
      // This would continue with player actions, advancing streets, etc.
      onProgress?.("Game ready for betting");

      return gameStatePDA as any;
    } catch (error) {
      onProgress?.("Error", { error });
      throw error;
    }
  }
}

