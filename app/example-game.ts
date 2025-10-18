/**
 * Example: Running a complete poker game
 * 
 * This script demonstrates:
 * 1. Setting up players
 * 2. Creating and joining a game
 * 3. Using the commit-reveal scheme
 * 4. Playing through all betting rounds
 * 5. Resolving the winner
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { TiltPrograms } from "../target/types/tilt_programs";
import { createPokerClient, PlayerAction } from "./poker-client";

async function main() {
  // Setup connection
  const connection = new Connection("http://localhost:8899", "confirmed");
  
  // Load program
  const provider = new AnchorProvider(
    connection,
    new Wallet(Keypair.generate()),
    { commitment: "confirmed" }
  );
  
  const idl = require("../target/idl/tilt_programs.json");
  const programId = new anchor.web3.PublicKey(idl.metadata.address);
  const program = new Program(idl, programId, provider) as Program<TiltPrograms>;
  
  // Create poker client
  const client = createPokerClient(program, provider);
  
  // Create players
  console.log("🎲 Creating players...");
  const player1 = Keypair.generate();
  const player2 = Keypair.generate();
  
  // Airdrop SOL for testing
  console.log("💰 Airdropping SOL to players...");
  const sig1 = await connection.requestAirdrop(
    player1.publicKey,
    2 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(sig1);
  
  const sig2 = await connection.requestAirdrop(
    player2.publicKey,
    2 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(sig2);
  
  // Initialize player accounts
  console.log("👤 Initializing player accounts...");
  await client.initializePlayer(player1);
  await client.initializePlayer(player2);
  
  // Create game
  console.log("\n🃏 Creating poker game...");
  const stakeAmount = 0.01 * LAMPORTS_PER_SOL; // 0.01 SOL
  const { gameStatePDA, secret: secret1 } = await client.createGame(
    player1,
    stakeAmount
  );
  console.log(`   Game created at: ${gameStatePDA.toString()}`);
  console.log(`   Stake: ${stakeAmount / LAMPORTS_PER_SOL} SOL`);
  
  // Player 2 joins
  console.log("\n🎮 Player 2 joining...");
  const { secret: secret2 } = await client.joinGame(player2, gameStatePDA);
  console.log("   Player 2 joined successfully");
  
  // Reveal secrets (commit-reveal for randomness)
  console.log("\n🔐 Revealing secrets for provably fair shuffle...");
  await client.revealSecret(player1, gameStatePDA, secret1);
  console.log("   Player 1 revealed secret");
  await client.revealSecret(player2, gameStatePDA, secret2);
  console.log("   Player 2 revealed secret");
  console.log("   ✓ Deck shuffled using combined entropy");
  
  // Deal cards
  console.log("\n🎴 Dealing cards...");
  await client.dealInitial(gameStatePDA);
  
  let gameState = await client.getGameState(gameStatePDA);
  console.log(`   Player 1 hand: ${client.formatCards(gameState.player1Hand)}`);
  console.log(`   Player 2 hand: ${client.formatCards(gameState.player2Hand)}`);
  console.log(`   Pot: ${gameState.pot.toNumber() / LAMPORTS_PER_SOL} SOL`);
  
  // Pre-flop betting
  console.log("\n💰 PRE-FLOP BETTING");
  gameState = await client.getGameState(gameStatePDA);
  const currentPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const currentPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} calls`);
  await client.playerAction(currentPlayer1, gameStatePDA, PlayerAction.Call);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks`);
  await client.playerAction(currentPlayer2, gameStatePDA, PlayerAction.Check);
  
  // Advance to flop
  console.log("\n🎴 FLOP");
  await client.advanceStreet(gameStatePDA);
  gameState = await client.getGameState(gameStatePDA);
  console.log(`   Community: ${client.formatCards(gameState.communityCards.slice(0, 3))}`);
  
  // Flop betting
  console.log("💰 Flop betting...");
  gameState = await client.getGameState(gameStatePDA);
  const flopPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const flopPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(flopPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks`);
  await client.playerAction(flopPlayer2, gameStatePDA, PlayerAction.Check);
  
  // Advance to turn
  console.log("\n🎴 TURN");
  await client.advanceStreet(gameStatePDA);
  gameState = await client.getGameState(gameStatePDA);
  console.log(`   Community: ${client.formatCards(gameState.communityCards.slice(0, 4))}`);
  
  // Turn betting
  console.log("💰 Turn betting...");
  gameState = await client.getGameState(gameStatePDA);
  const turnPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const turnPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(turnPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks`);
  await client.playerAction(turnPlayer2, gameStatePDA, PlayerAction.Check);
  
  // Advance to river
  console.log("\n🎴 RIVER");
  await client.advanceStreet(gameStatePDA);
  gameState = await client.getGameState(gameStatePDA);
  console.log(`   Community: ${client.formatCards(gameState.communityCards)}`);
  
  // River betting
  console.log("💰 River betting...");
  gameState = await client.getGameState(gameStatePDA);
  const riverPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const riverPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(riverPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks`);
  await client.playerAction(riverPlayer2, gameStatePDA, PlayerAction.Check);
  
  // Advance to showdown
  console.log("\n🏆 SHOWDOWN");
  await client.advanceStreet(gameStatePDA);
  
  // Resolve game
  console.log("🎯 Resolving game...");
  await client.resolveGame(
    gameStatePDA,
    player1.publicKey,
    player2.publicKey
  );
  
  // Display results
  gameState = await client.getGameState(gameStatePDA);
  console.log("\n" + "=".repeat(60));
  console.log("                    GAME RESULTS");
  console.log("=".repeat(60));
  console.log(`\n👤 Player 1:`);
  console.log(`   Hand: ${client.formatCards(gameState.player1Hand)}`);
  console.log(`\n👤 Player 2:`);
  console.log(`   Hand: ${client.formatCards(gameState.player2Hand)}`);
  console.log(`\n🎴 Community Cards: ${client.formatCards(gameState.communityCards)}`);
  
  if (gameState.winner) {
    const winnerId = gameState.winner.toString() === player1.publicKey.toString() ? 1 : 2;
    console.log(`\n🏆 Winner: Player ${winnerId}`);
    console.log(`   Hand: ${client.getHandRankName(gameState.winningHandRank || 0)}`);
    console.log(`   Pot: ${gameState.pot.toNumber() / LAMPORTS_PER_SOL} SOL`);
  } else {
    console.log("\n🤝 Result: Split Pot");
    console.log(`   Each player gets: ${gameState.pot.toNumber() / 2 / LAMPORTS_PER_SOL} SOL`);
  }
  
  // Display player stats
  console.log("\n" + "=".repeat(60));
  const player1Account = await client.getPlayerAccount(player1.publicKey);
  const player2Account = await client.getPlayerAccount(player2.publicKey);
  
  console.log("\n📊 Player 1 Stats:");
  console.log(`   Hands Played: ${player1Account.totalHandsPlayed}`);
  console.log(`   Hands Won: ${player1Account.totalHandsWon}`);
  console.log(`   Total Winnings: ${player1Account.totalWinnings / LAMPORTS_PER_SOL} SOL`);
  
  console.log("\n📊 Player 2 Stats:");
  console.log(`   Hands Played: ${player2Account.totalHandsPlayed}`);
  console.log(`   Hands Won: ${player2Account.totalHandsWon}`);
  console.log(`   Total Winnings: ${player2Account.totalWinnings / LAMPORTS_PER_SOL} SOL`);
  
  console.log("\n" + "=".repeat(60));
  console.log("✅ Game completed successfully!");
  console.log("=".repeat(60) + "\n");
}

// Run if called directly
if (require.main === module) {
  main()
    .then(() => process.exit(0))
    .catch((error) => {
      console.error("Error:", error);
      process.exit(1);
    });
}

export { main };

