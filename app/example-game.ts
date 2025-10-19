/**
 * Example: Running a complete poker game with USDC
 * 
 * This script demonstrates:
 * 1. Setting up test USDC mint and token accounts
 * 2. Initializing players and balances
 * 3. Depositing USDC into player balances
 * 4. Creating and joining a game with USDC stakes
 * 5. Using the commit-reveal scheme for provably fair shuffling
 * 6. Playing through all betting rounds
 * 7. Resolving the winner and distributing USDC winnings
 * 8. Withdrawing USDC back to token accounts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey } from "@solana/web3.js";
import { 
  TOKEN_PROGRAM_ID, 
  createMint, 
  createAccount,
  mintTo,
  getAccount,
  getMinimumBalanceForRentExemptAccount,
  createInitializeAccountInstruction,
  ACCOUNT_SIZE
} from "@solana/spl-token";
import { Transaction, SystemProgram } from "@solana/web3.js";
import { TiltPrograms } from "../target/types/tilt_programs";
import { createPokerClient, PlayerAction } from "./poker-client";

async function main() {
  // Setup connection
  const connection = new Connection("http://localhost:8899", "confirmed");
  
  // Create players FIRST (before provider)
  console.log("🎲 Creating players...");
  const player1 = Keypair.generate();
  const player2 = Keypair.generate();
  const mintAuthority = Keypair.generate();
  
  // Airdrop SOL for testing (for transaction fees)
  console.log("💰 Airdropping SOL for transaction fees...");
  const sig1 = await connection.requestAirdrop(
    player1.publicKey,
    2 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(sig1, "confirmed");
  console.log("   Player 1 funded");
  
  const sig2 = await connection.requestAirdrop(
    player2.publicKey,
    2 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(sig2, "confirmed");
  console.log("   Player 2 funded");
  
  const sig3 = await connection.requestAirdrop(
    mintAuthority.publicKey,
    2 * LAMPORTS_PER_SOL
  );
  await connection.confirmTransaction(sig3, "confirmed");
  console.log("   Mint authority funded");
  
  // Wait a bit for balances to update
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  // Create provider with player1 as the wallet (for paying fees)
  const provider = new AnchorProvider(
    connection,
    new Wallet(player1),
    { commitment: "confirmed" }
  );
  
  const idl = require("../target/idl/tilt_programs.json");
  // Get program ID from IDL or use the one from declare_id in lib.rs
  const programId = new anchor.web3.PublicKey(
    idl.address || idl.metadata?.address || "5mqXj7QV7SGLsJ3n6UCeau4sd7aDtaBH7E9WaHoiGKHA"
  );
  const program = new Program(idl, programId, provider) as Program<TiltPrograms>;
  
  // Create test USDC mint (for local testing)
  console.log("\n💵 Creating test USDC mint...");
  const usdcMint = await createMint(
    connection,
    mintAuthority,
    mintAuthority.publicKey,
    null,
    6 // USDC has 6 decimals
  );
  console.log(`   USDC Mint: ${usdcMint.toString()}`);
  
  // Create token accounts for players
  console.log("\n🏦 Creating token accounts...");
  const player1TokenAccount = await createAccount(
    connection,
    player1,
    usdcMint,
    player1.publicKey
  );
  console.log("   Player 1 token account created");
  
  const player2TokenAccount = await createAccount(
    connection,
    player2,
    usdcMint,
    player2.publicKey
  );
  console.log("   Player 2 token account created");
  
  // Create program vault token account (for PDA, we need to do this manually)
  const [programVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("program_vault")],
    program.programId
  );
  
  // Generate a new account for the program vault's token account
  const programVaultTokenAccount = Keypair.generate();
  
  // Get rent exemption
  const rentExemption = await getMinimumBalanceForRentExemptAccount(connection);
  
  // Create the token account manually
  const createAccountIx = SystemProgram.createAccount({
    fromPubkey: mintAuthority.publicKey,
    newAccountPubkey: programVaultTokenAccount.publicKey,
    lamports: rentExemption,
    space: ACCOUNT_SIZE,
    programId: TOKEN_PROGRAM_ID,
  });
  
  const initAccountIx = createInitializeAccountInstruction(
    programVaultTokenAccount.publicKey,
    usdcMint,
    programVaultPDA, // The PDA is the owner
    TOKEN_PROGRAM_ID
  );
  
  const tx = new Transaction().add(createAccountIx, initAccountIx);
  await provider.sendAndConfirm(tx, [mintAuthority, programVaultTokenAccount]);
  console.log("   Program vault token account created");
  
  // Mint test USDC to players (1000 USDC each)
  console.log("\n💸 Minting test USDC to players...");
  const mintAmount = 1000 * 1_000_000; // 1000 USDC with 6 decimals
  
  await mintTo(
    connection,
    mintAuthority,
    usdcMint,
    player1TokenAccount,
    mintAuthority,
    mintAmount
  );
  console.log("   Player 1: 1000 USDC");
  
  await mintTo(
    connection,
    mintAuthority,
    usdcMint,
    player2TokenAccount,
    mintAuthority,
    mintAmount
  );
  console.log("   Player 2: 1000 USDC");
  
  // Store program vault token account address in client
  // In production, this would be stored in program state or derived deterministically
  console.log(`\n🔐 Program Vault Token Account: ${programVaultTokenAccount.publicKey.toString()}`);
  
  // Create poker client with USDC mint and program vault token account
  const client = createPokerClient(program, provider, usdcMint, programVaultTokenAccount.publicKey);
  
  // Initialize player accounts
  console.log("\n👤 Initializing player accounts...");
  await client.initializePlayer(player1);
  await client.initializePlayer(player2);
  console.log("   Player accounts initialized");
  
  // Initialize player balances
  console.log("\n💰 Initializing player balances...");
  await client.initializeBalance(player1);
  await client.initializeBalance(player2);
  console.log("   Balance accounts initialized");
  
  // Deposit USDC into player balances
  console.log("\n📥 Depositing USDC into player balances...");
  const depositAmount = 100 * 1_000_000; // 100 USDC
  await client.depositFunds(player1, depositAmount);
  console.log("   Player 1 deposited 100 USDC");
  
  await client.depositFunds(player2, depositAmount);
  console.log("   Player 2 deposited 100 USDC");
  
  // Check balances
  const balance1 = await client.getPlayerBalance(player1.publicKey);
  const balance2 = await client.getPlayerBalance(player2.publicKey);
  console.log(`   Player 1 balance: ${balance1.balance / 1_000_000} USDC`);
  console.log(`   Player 2 balance: ${balance2.balance / 1_000_000} USDC`);
  
  // Create game
  console.log("\n🃏 Creating poker game...");
  const stakeAmount = 10 * 1_000_000; // 10 USDC
  const { gameStatePDA, secret: secret1 } = await client.createGame(
    player1,
    stakeAmount
  );
  console.log(`   Game created at: ${gameStatePDA.toString()}`);
  console.log(`   Stake: ${stakeAmount / 1_000_000} USDC`);
  
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
  console.log(`   Player 1 stack: ${gameState.player1Stack.toNumber() / 1_000_000} USDC`);
  console.log(`   Player 2 stack: ${gameState.player2Stack.toNumber() / 1_000_000} USDC`);
  console.log(`   Pot: ${gameState.pot.toNumber() / 1_000_000} USDC`);
  
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
    console.log(`   Pot: ${gameState.pot.toNumber() / 1_000_000} USDC`);
  } else {
    console.log("\n🤝 Result: Split Pot");
    console.log(`   Each player gets: ${gameState.pot.toNumber() / 2 / 1_000_000} USDC`);
  }
  
  // Display player stats and balances
  console.log("\n" + "=".repeat(60));
  const player1Account = await client.getPlayerAccount(player1.publicKey);
  const player2Account = await client.getPlayerAccount(player2.publicKey);
  const finalBalance1 = await client.getPlayerBalance(player1.publicKey);
  const finalBalance2 = await client.getPlayerBalance(player2.publicKey);
  
  console.log("\n📊 Player 1 Stats:");
  console.log(`   Hands Played: ${player1Account.totalHandsPlayed}`);
  console.log(`   Hands Won: ${player1Account.totalHandsWon}`);
  console.log(`   Total Winnings: ${player1Account.totalWinnings / 1_000_000} USDC`);
  console.log(`   Current Balance: ${finalBalance1.balance / 1_000_000} USDC`);
  
  console.log("\n📊 Player 2 Stats:");
  console.log(`   Hands Played: ${player2Account.totalHandsPlayed}`);
  console.log(`   Hands Won: ${player2Account.totalHandsWon}`);
  console.log(`   Total Winnings: ${player2Account.totalWinnings / 1_000_000} USDC`);
  console.log(`   Current Balance: ${finalBalance2.balance / 1_000_000} USDC`);
  
  // Demonstrate withdrawal
  console.log("\n💸 Demonstrating withdrawal...");
  const withdrawAmount = 10 * 1_000_000; // Withdraw 10 USDC
  if (gameState.winner) {
    const winner = gameState.winner.toString() === player1.publicKey.toString() ? player1 : player2;
    const winnerNum = winner === player1 ? 1 : 2;
    console.log(`   Player ${winnerNum} withdrawing 10 USDC...`);
    await client.withdrawFunds(winner, withdrawAmount);
    
    const tokenAccount = winner === player1 ? player1TokenAccount : player2TokenAccount;
    const tokenAccountInfo = await getAccount(connection, tokenAccount);
    console.log(`   Withdrawn! Token account balance: ${Number(tokenAccountInfo.amount) / 1_000_000} USDC`);
  }
  
  console.log("\n" + "=".repeat(60));
  console.log("✅ Game completed successfully with USDC!");
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

