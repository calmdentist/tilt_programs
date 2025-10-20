import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { Zkpoker } from "../target/types/zkpoker";
import { createPokerClient, PlayerAction, GameFlow } from "../app/poker-client";
import { expect } from "chai";

describe("Poker Game Flow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Zkpoker as Program<Zkpoker>;
  const client = createPokerClient(program, provider);

  let player1: Keypair;
  let player2: Keypair;

  beforeEach(async () => {
    // Create fresh players for each test
    player1 = Keypair.generate();
    player2 = Keypair.generate();

    // Airdrop SOL for testing
    const airdrop1 = await provider.connection.requestAirdrop(
      player1.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop1);

    const airdrop2 = await provider.connection.requestAirdrop(
      player2.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdrop2);
  });

  it("Initializes player accounts", async () => {
    await client.initializePlayer(player1);
    await client.initializePlayer(player2);

    const player1Account = await client.getPlayerAccount(player1.publicKey);
    const player2Account = await client.getPlayerAccount(player2.publicKey);

    expect(player1Account.authority.toString()).to.equal(
      player1.publicKey.toString()
    );
    expect(player2Account.authority.toString()).to.equal(
      player2.publicKey.toString()
    );
    expect(player1Account.totalHandsPlayed.toNumber()).to.equal(0);
  });

  it("Creates and joins a game", async () => {
    const stakeAmount = 1000000; // 0.001 SOL

    // Create game
    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );

    let gameState = await client.getGameState(gameStatePDA);
    expect(gameState.player1.toString()).to.equal(player1.publicKey.toString());
    expect(gameState.stakeAmount.toNumber()).to.equal(stakeAmount);

    // Join game
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    gameState = await client.getGameState(gameStatePDA);
    expect(gameState.player2.toString()).to.equal(player2.publicKey.toString());
  });

  it("Reveals secrets and shuffles deck", async () => {
    const stakeAmount = 1000000;

    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    // Reveal secrets
    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);

    const gameState = await client.getGameState(gameStatePDA);
    expect(gameState.player1SecretRevealed).to.be.true;
    expect(gameState.player2SecretRevealed).to.be.true;

    // Check that deck is shuffled (not in original order)
    const deck = gameState.deck;
    let isDifferent = false;
    for (let i = 0; i < 52; i++) {
      if (deck[i] !== i) {
        isDifferent = true;
        break;
      }
    }
    expect(isDifferent).to.be.true;
  });

  it("Deals initial cards", async () => {
    const stakeAmount = 1000000;

    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);
    await client.dealInitial(gameStatePDA);

    const gameState = await client.getGameState(gameStatePDA);
    
    // Each player should have 2 cards
    expect(gameState.player1Hand).to.have.lengthOf(2);
    expect(gameState.player2Hand).to.have.lengthOf(2);

    console.log("Player 1 hand:", client.formatCards(gameState.player1Hand));
    console.log("Player 2 hand:", client.formatCards(gameState.player2Hand));
  });

  it("Handles betting actions", async () => {
    const stakeAmount = 1000000;

    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);
    await client.dealInitial(gameStatePDA);

    let gameState = await client.getGameState(gameStatePDA);
    const currentPlayer =
      gameState.currentPlayer === 1 ? player1 : player2;

    // Call the big blind
    await client.playerAction(
      currentPlayer,
      gameStatePDA,
      PlayerAction.Call
    );

    gameState = await client.getGameState(gameStatePDA);
    
    // Current player should have switched
    const newCurrentPlayer = gameState.currentPlayer === 1 ? 1 : 2;
    expect(newCurrentPlayer).to.not.equal(
      currentPlayer === player1 ? 1 : 2
    );
  });

  it("Advances through streets", async () => {
    const stakeAmount = 1000000;

    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);
    await client.dealInitial(gameStatePDA);

    let gameState = await client.getGameState(gameStatePDA);
    
    // Complete pre-flop betting (both check)
    const firstPlayer = gameState.currentPlayer === 1 ? player1 : player2;
    const secondPlayer = gameState.currentPlayer === 1 ? player2 : player1;
    
    await client.playerAction(firstPlayer, gameStatePDA, PlayerAction.Call);
    await client.playerAction(secondPlayer, gameStatePDA, PlayerAction.Check);

    // Advance to flop
    await client.advanceStreet(gameStatePDA);

    gameState = await client.getGameState(gameStatePDA);
    expect(gameState.communityCardsDealt).to.equal(3);
    
    console.log("Flop:", client.formatCards(gameState.communityCards.slice(0, 3)));
  });

  it("Completes a full game", async () => {
    // Initialize players
    await client.initializePlayer(player1);
    await client.initializePlayer(player2);

    const stakeAmount = 1000000;
    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);
    await client.dealInitial(gameStatePDA);

    // Pre-flop: both check/call
    let gameState = await client.getGameState(gameStatePDA);
    const p1First = gameState.currentPlayer === 1;
    
    await client.playerAction(
      p1First ? player1 : player2,
      gameStatePDA,
      PlayerAction.Call
    );
    await client.playerAction(
      p1First ? player2 : player1,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.advanceStreet(gameStatePDA);

    // Flop: both check
    gameState = await client.getGameState(gameStatePDA);
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.advanceStreet(gameStatePDA);

    // Turn: both check
    gameState = await client.getGameState(gameStatePDA);
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.advanceStreet(gameStatePDA);

    // River: both check
    gameState = await client.getGameState(gameStatePDA);
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.playerAction(
      gameState.currentPlayer === 1 ? player1 : player2,
      gameStatePDA,
      PlayerAction.Check
    );
    await client.advanceStreet(gameStatePDA);

    // Resolve
    await client.resolveGame(
      gameStatePDA,
      player1.publicKey,
      player2.publicKey
    );

    gameState = await client.getGameState(gameStatePDA);
    
    console.log("\n=== GAME RESULT ===");
    console.log("Player 1 hand:", client.formatCards(gameState.player1Hand));
    console.log("Player 2 hand:", client.formatCards(gameState.player2Hand));
    console.log("Community:", client.formatCards(gameState.communityCards));
    
    if (gameState.winner) {
      console.log("Winner:", gameState.winner.toString());
      console.log(
        "Hand rank:",
        client.getHandRankName(gameState.winningHandRank || 0)
      );
    } else {
      console.log("Result: Split pot");
    }

    // Check player stats updated
    const player1Account = await client.getPlayerAccount(player1.publicKey);
    expect(player1Account.totalHandsPlayed.toNumber()).to.equal(1);
  });

  it("Handles fold action", async () => {
    const stakeAmount = 1000000;

    const { gameStatePDA, secret: secret1 } = await client.createGame(
      player1,
      stakeAmount
    );
    const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);

    await client.revealSecret(player1, gameStatePDA, secret1);
    await client.revealSecret(player2, gameStatePDA, secret2);
    await client.dealInitial(gameStatePDA);

    let gameState = await client.getGameState(gameStatePDA);
    const currentPlayer = gameState.currentPlayer === 1 ? player1 : player2;
    const otherPlayer = currentPlayer === player1 ? player2 : player1;

    // Current player folds
    await client.playerAction(currentPlayer, gameStatePDA, PlayerAction.Fold);

    gameState = await client.getGameState(gameStatePDA);
    expect(gameState.winner?.toString()).to.equal(otherPlayer.publicKey.toString());
  });

  it("Uses GameFlow helper for quick setup", async () => {
    const gameFlow = new GameFlow(client);
    const stakeAmount = 1000000;

    const logs: string[] = [];
    const gameStatePDA = await gameFlow.runCompleteGame(
      player1,
      player2,
      stakeAmount,
      (stage, data) => {
        console.log(`[${stage}]`, data || "");
        logs.push(stage);
      }
    );

    expect(logs).to.include("Game created");
    expect(logs).to.include("Player 2 joined");
    expect(logs).to.include("Secrets revealed, deck shuffled");
    expect(logs).to.include("Cards dealt");
  });
});

