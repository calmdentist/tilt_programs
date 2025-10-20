/**
 * Complete Mental Poker Game Example
 * 
 * Demonstrates a full game with actual Pohlig-Hellman encryption:
 * 1. Player 1 creates game with shuffled + encrypted deck
 * 2. Player 2 joins with Merkle proofs
 * 3. Pre-flop betting
 * 4. Flop reveal (progressive two-step)
 * 5. Flop betting
 * 6. Turn reveal
 * 7. Turn betting  
 * 8. River reveal
 * 9. River betting
 * 10. Showdown with hand reveals
 * 11. Winner determination and payout
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, ComputeBudgetProgram } from "@solana/web3.js";
import { 
  TOKEN_PROGRAM_ID, 
  createMint, 
  createAccount,
  mintTo,
  getMinimumBalanceForRentExemptAccount,
  createInitializeAccountInstruction,
  ACCOUNT_SIZE
} from "@solana/spl-token";
import { Transaction, SystemProgram } from "@solana/web3.js";
import { Zkpoker } from "../target/types/zkpoker";
import { createPokerClient, PlayerAction, MentalPokerCrypto, Card } from "./poker-client";
import * as fs from "fs";

async function main() {
  console.log("\n" + "=".repeat(80));
  console.log("🎴 MENTAL POKER - Complete Game Example");
  console.log("=".repeat(80) + "\n");

  // Setup
  const connection = new Connection("http://localhost:8899", "confirmed");
  
  console.log("👥 Setting up players...");
  const player1 = Keypair.generate();
  const player2 = Keypair.generate();
  
  // Load deterministic USDC mint keypair
  const usdcMintKeypairPath = __dirname + "/../keypairs/usdc-mint-keypair.json";
  const usdcMintKeypairData = JSON.parse(fs.readFileSync(usdcMintKeypairPath, "utf-8"));
  const usdcMintKeypair = Keypair.fromSecretKey(new Uint8Array(usdcMintKeypairData));
  
  // Mint authority can be player1 (doesn't need to be deterministic)
  const mintAuthority = player1;
  
  // Airdrop SOL
  console.log("💰 Airdropping SOL...");
  await Promise.all([
    connection.requestAirdrop(player1.publicKey, 3 * LAMPORTS_PER_SOL).then(sig => connection.confirmTransaction(sig, "confirmed")),
    connection.requestAirdrop(player2.publicKey, 3 * LAMPORTS_PER_SOL).then(sig => connection.confirmTransaction(sig, "confirmed")),
  ]);
  console.log("   ✓ Funded\n");
  
  await new Promise(resolve => setTimeout(resolve, 1000));
  
  // Setup program
  const provider = new AnchorProvider(connection, new Wallet(player1), { commitment: "confirmed" });
  const idl = require("../target/idl/zkpoker.json");
  const programId = new anchor.web3.PublicKey(idl.address || idl.metadata?.address);
  const program = new Program(idl, programId, provider) as Program<Zkpoker>;
  
  // Create USDC mint with deterministic address
  console.log("💵 Creating test USDC mint (deterministic)...");
  const usdcMint = await createMint(connection, player1, mintAuthority.publicKey, null, 6, usdcMintKeypair);
  
  const player1TokenAccount = await createAccount(connection, player1, usdcMint, player1.publicKey);
  const player2TokenAccount = await createAccount(connection, player2, usdcMint, player2.publicKey);
  
  // Program vault
  const [programVaultPDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("program_vault")],
    program.programId
  );
  
  const programVaultTokenAccount = Keypair.generate();
  const rentExemption = await getMinimumBalanceForRentExemptAccount(connection);
  
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
    programVaultPDA,
    TOKEN_PROGRAM_ID
  );
  
  const tx = new Transaction().add(createAccountIx, initAccountIx);
  await provider.sendAndConfirm(tx, [mintAuthority, programVaultTokenAccount]);
  
  // Mint USDC
  const mintAmount = 1000 * 1_000_000;
  await Promise.all([
    mintTo(connection, mintAuthority, usdcMint, player1TokenAccount, mintAuthority, mintAmount),
    mintTo(connection, mintAuthority, usdcMint, player2TokenAccount, mintAuthority, mintAmount),
  ]);
  console.log("   ✓ USDC setup complete\n");
  
  // Create client
  const client = createPokerClient(program, provider, usdcMint, programVaultTokenAccount.publicKey);
  
  // Initialize players
  console.log("📋 Initializing accounts...");
  await client.initializePlayer(player1);
  await client.initializePlayer(player2);
  await client.initializeBalance(player1);
  await client.initializeBalance(player2);
  
  const depositAmount = 100 * 1_000_000;
  await client.depositFunds(player1, depositAmount);
  await client.depositFunds(player2, depositAmount);
  console.log("   ✓ Players initialized with 100 USDC each\n");
  
  console.log("=".repeat(80));
  console.log("                        PHASE 1: GAME SETUP");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 1: GAME SETUP ===
  console.log("🔑 Player 1 generating keypair...");
  const stakeAmount = 10 * 1_000_000;
  
  const { 
    gameStatePDA, 
    player1Keypair, 
    encryptedDeck, 
    merkleRoot 
  } = await client.createGame(player1, stakeAmount);
  
  console.log("   ✓ Ephemeral keypair generated");
  console.log("   ✓ 52-card deck shuffled");
  console.log("   ✓ Deck encrypted with Player 1's key");
  console.log("   ✓ Merkle root computed and committed");
  console.log(`   📍 Game: ${gameStatePDA.toString().slice(0, 16)}...`);
  console.log(`   💰 Stake: ${stakeAmount / 1_000_000} USDC (+ 1 USDC bond)\n`);
  
  console.log("🔑 Player 2 joining...");
  console.log("   1️⃣  Receiving Player 1's encrypted deck");
  console.log("   2️⃣  Generating ephemeral keypair");
  console.log("   3️⃣  Re-shuffling encrypted deck");
  console.log("   4️⃣  Selecting 9 cards (2+2 pocket cards + 5 community)");
  console.log("   5️⃣  Double-encrypting with Player 2's key");
  console.log("   6️⃣  Generating Merkle proofs (log₂(52) = 6 proofs per card)");
  console.log("   7️⃣  Submitting to chain...\n");
  
  console.log("   ⚠️  OPTIMISTIC VERIFICATION MODEL:");
  console.log("       On-chain: Just store encrypted cards (no verification)");
  console.log("       Off-chain: Player 1 verifies Merkle proofs client-side");
  console.log("       Security: Player 2's bond at stake, Player 1 can exit if invalid\n");
  
  // Join game - returns proof data for off-chain verification
  const { player2Keypair, proofData } = await client.joinGame(player2, gameStatePDA, encryptedDeck);
  
  console.log("   ✓ Player 2 joined on-chain (submitted encrypted cards)");
  console.log("   ✓ Blinds posted (0.5 + 1 USDC)\n");
  
  // Player 1 MUST verify off-chain before continuing
  console.log("🔍 Player 1 verifying Player 2's deck selection OFF-CHAIN...");
  const verification = MentalPokerCrypto.verifyDeckSelection(
    encryptedDeck,
    player2Keypair.publicKey,
    proofData.singlyEncryptedCards,
    proofData.doublyEncryptedCards,
    merkleRoot,
    proofData.merkleProofs
  );
  
  if (!verification.valid) {
    console.log(`   ❌ VERIFICATION FAILED: ${verification.reason}`);
    console.log("   → Player 1 should call claim_timeout() to get stake + Player 2's bond");
    console.log("   → Player 2 loses their bond for cheating!\n");
    process.exit(1);
  }
  
  console.log("   ✓ All 9 Merkle proofs valid");
  console.log("   ✓ All cards from Player 1's committed deck");
  console.log("   ✓ Double encryption correct");
  console.log("   ✓ Deck integrity CONFIRMED - game can proceed safely!\n");
  
  let gameState = await client.getGameState(gameStatePDA);
  console.log(`   💰 Pot: ${gameState.pot.toNumber() / 1_000_000} USDC`);
  console.log(`   🎯 Stage: Pre-Flop Betting\n`);
  
  console.log("=".repeat(80));
  console.log("                        PHASE 2: PRE-FLOP");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 2: PRE-FLOP BETTING ===
  console.log("💰 Pre-Flop Betting Round:");
  gameState = await client.getGameState(gameStatePDA);
  
  const currentPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const currentPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} calls (1 USDC)`);
  await client.playerAction(currentPlayer1, gameStatePDA, PlayerAction.Call);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks`);
  await client.playerAction(currentPlayer2, gameStatePDA, PlayerAction.Check);
  
  gameState = await client.getGameState(gameStatePDA);
  console.log(`   💰 Pot: ${gameState.pot.toNumber() / 1_000_000} USDC\n`);
  
  console.log("=".repeat(80));
  console.log("                        PHASE 3: FLOP REVEAL");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 3: FLOP ===
  console.log("📍 Transitioning to Flop...");
  await client.advanceStreet(gameStatePDA);
  console.log("   ✓ Stage: Awaiting Flop Reveal\n");
  
  console.log("🔓 Two-Step Flop Reveal Process:\n");
  
  console.log("   Step 1: Player 1 decrypts with their private key");
  gameState = await client.getGameState(gameStatePDA);
  
  // Player 1 decrypts the 3 flop cards (indices 4, 5, 6)
  const flopIndices = [4, 5, 6];
  const player1FlopShares = flopIndices.map(idx => {
    const doublyEncrypted = gameState.encryptedCards[idx].data;
    return MentalPokerCrypto.decryptOneLayer(doublyEncrypted, player1Keypair.privateKey);
  });
  
  console.log("   → Player 1 submitting decryption shares...");
  await client.revealCommunityCards(player1, gameStatePDA, player1FlopShares);
  console.log("   ✓ Player 1 shares submitted\n");
  
  console.log("   Step 2: Player 2 completes decryption + provides plaintext");
  gameState = await client.getGameState(gameStatePDA);
  
  // Player 2 decrypts to get plaintext
  const player2FlopShares = flopIndices.map((_, idx) => {
    return MentalPokerCrypto.decryptOneLayer(
      player1FlopShares[idx],
      player2Keypair.privateKey
    );
  });
  
  // After decryptOneLayer, player2FlopShares contains the plaintext (card + 2) as bytes
  // Convert from bytes to card number
  const flopPlaintext = player2FlopShares.map(share => {
    const plaintextBigInt = MentalPokerCrypto.bytesToBigInt(new Uint8Array(share));
    const card = Number(plaintextBigInt) - 2; // Subtract the offset we added during encryption
    if (card < 0 || card > 51) {
      throw new Error(`Invalid card value after decryption: ${card}`);
    }
    return card;
  });
  
  console.log("   → Player 2 submitting shares + plaintext for verification...");
  await client.revealCommunityCards(player2, gameStatePDA, player2FlopShares, flopPlaintext);
  console.log("   ✓ Player 2 shares submitted");
  console.log("   ✓ On-chain verification: PASSED\n");
  
  gameState = await client.getGameState(gameStatePDA);
  const flopCards = flopPlaintext.map(c => new Card(c).toString()).join(" ");
  console.log(`   🎴 FLOP: ${flopCards}\n`);
  
  console.log("💰 Flop Betting Round:");
  const flopPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const flopPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(flopPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks\n`);
  await client.playerAction(flopPlayer2, gameStatePDA, PlayerAction.Check);
  
  console.log("=".repeat(80));
  console.log("                        PHASE 4: TURN REVEAL");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 4: TURN ===
  console.log("📍 Transitioning to Turn...");
  await client.advanceStreet(gameStatePDA);
  
  console.log("🔓 Two-Step Turn Reveal Process:\n");
  
  gameState = await client.getGameState(gameStatePDA);
  const turnIdx = 7;
  
  const player1TurnShare = MentalPokerCrypto.decryptOneLayer(
    gameState.encryptedCards[turnIdx].data,
    player1Keypair.privateKey
  );
  
  console.log("   → Player 1 submitting turn decryption share...");
  await client.revealCommunityCards(player1, gameStatePDA, [player1TurnShare]);
  console.log("   ✓ Player 1 share submitted\n");
  
  const player2TurnShare = MentalPokerCrypto.decryptOneLayer(
    player1TurnShare,
    player2Keypair.privateKey
  );
  
  // Convert from bytes to card number
  const turnPlaintextBigInt = MentalPokerCrypto.bytesToBigInt(new Uint8Array(player2TurnShare));
  const turnPlaintext = Number(turnPlaintextBigInt) - 2;
  
  console.log("   → Player 2 completing decryption...");
  await client.revealCommunityCards(player2, gameStatePDA, [player2TurnShare], [turnPlaintext]);
  console.log("   ✓ Verification: PASSED\n");
  
  const turnCard = new Card(turnPlaintext).toString();
  console.log(`   🎴 TURN: ${flopCards} ${turnCard}\n`);
  
  console.log("💰 Turn Betting Round:");
  gameState = await client.getGameState(gameStatePDA);
  const turnPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const turnPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(turnPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks\n`);
  await client.playerAction(turnPlayer2, gameStatePDA, PlayerAction.Check);
  
  console.log("=".repeat(80));
  console.log("                        PHASE 5: RIVER REVEAL");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 5: RIVER ===
  console.log("📍 Transitioning to River...");
  await client.advanceStreet(gameStatePDA);
  
  console.log("🔓 Two-Step River Reveal Process:\n");
  
  gameState = await client.getGameState(gameStatePDA);
  const riverIdx = 8;
  
  const player1RiverShare = MentalPokerCrypto.decryptOneLayer(
    gameState.encryptedCards[riverIdx].data,
    player1Keypair.privateKey
  );
  
  console.log("   → Player 1 submitting river decryption share...");
  await client.revealCommunityCards(player1, gameStatePDA, [player1RiverShare]);
  console.log("   ✓ Player 1 share submitted\n");
  
  const player2RiverShare = MentalPokerCrypto.decryptOneLayer(
    player1RiverShare,
    player2Keypair.privateKey
  );
  
  // Convert from bytes to card number
  const riverPlaintextBigInt = MentalPokerCrypto.bytesToBigInt(new Uint8Array(player2RiverShare));
  const riverPlaintext = Number(riverPlaintextBigInt) - 2;
  
  console.log("   → Player 2 completing decryption...");
  await client.revealCommunityCards(player2, gameStatePDA, [player2RiverShare], [riverPlaintext]);
  console.log("   ✓ Verification: PASSED\n");
  
  const riverCard = new Card(riverPlaintext).toString();
  console.log(`   🎴 RIVER: ${flopCards} ${turnCard} ${riverCard}\n`);
  
  console.log("💰 River Betting Round:");
  gameState = await client.getGameState(gameStatePDA);
  const riverPlayer1 = gameState.currentPlayer === 1 ? player1 : player2;
  const riverPlayer2 = gameState.currentPlayer === 1 ? player2 : player1;
  
  console.log(`   Player ${gameState.currentPlayer} checks`);
  await client.playerAction(riverPlayer1, gameStatePDA, PlayerAction.Check);
  
  console.log(`   Player ${gameState.currentPlayer === 1 ? 2 : 1} checks\n`);
  await client.playerAction(riverPlayer2, gameStatePDA, PlayerAction.Check);
  
  console.log("=".repeat(80));
  console.log("                        PHASE 6: SHOWDOWN");
  console.log("=".repeat(80) + "\n");
  
  // === PHASE 6: SHOWDOWN ===
  console.log("📍 Transitioning to Showdown...");
  await client.advanceStreet(gameStatePDA);
  console.log("   ✓ Stage: Showdown - Players must reveal pocket cards\n");
  
  console.log("🔓 Two-Step Showdown Process:\n");
  
  console.log("   → Player 1 revealing pocket cards...");
  await client.resolveHand(player1, gameStatePDA, player1.publicKey, player2.publicKey);
  console.log("   ✓ Player 1 pocket cards verified\n");
  
  console.log("   → Player 2 revealing pocket cards...");
  await client.resolveHand(player2, gameStatePDA, player1.publicKey, player2.publicKey);
  console.log("   ✓ Player 2 pocket cards verified");
  console.log("   ✓ Hands evaluated on-chain\n");
  
  console.log("=".repeat(80));
  console.log("                        GAME RESULTS");
  console.log("=".repeat(80) + "\n");
  
  gameState = await client.getGameState(gameStatePDA);
  
  // Decrypt pocket cards for display
  const player1PocketIndices = [0, 1];
  const player2PocketIndices = [2, 3];
  
  const player1Pocket = player1PocketIndices.map(idx => {
    const doublyEncrypted = gameState.encryptedCards[idx].data;
    const singly = MentalPokerCrypto.decryptOneLayer(doublyEncrypted, player2Keypair.privateKey);
    return MentalPokerCrypto.decryptCard(singly, player1Keypair.privateKey);
  });
  
  const player2Pocket = player2PocketIndices.map(idx => {
    const doublyEncrypted = gameState.encryptedCards[idx].data;
    const singly = MentalPokerCrypto.decryptOneLayer(doublyEncrypted, player1Keypair.privateKey);
    return MentalPokerCrypto.decryptCard(singly, player2Keypair.privateKey);
  });
  
  const player1PocketStr = player1Pocket.map(c => new Card(c).toString()).join(" ");
  const player2PocketStr = player2Pocket.map(c => new Card(c).toString()).join(" ");
  
  console.log("👤 Player 1:");
  console.log(`   Pocket: ${player1PocketStr}`);
  console.log(`   Board:  ${flopCards} ${turnCard} ${riverCard}\n`);
  
  console.log("👤 Player 2:");
  console.log(`   Pocket: ${player2PocketStr}`);
  console.log(`   Board:  ${flopCards} ${turnCard} ${riverCard}\n`);
  
  if (gameState.winner) {
    const winnerId = gameState.winner.toString() === player1.publicKey.toString() ? 1 : 2;
    console.log(`🏆 WINNER: Player ${winnerId}`);
    console.log(`   💰 Won: ${gameState.pot.toNumber() / 1_000_000} USDC + 2 USDC (bonds returned)\n`);
  } else {
    console.log("🤝 Result: Tie - Pot Split\n");
  }
  
  const balance1 = await client.getPlayerBalance(player1.publicKey);
  const balance2 = await client.getPlayerBalance(player2.publicKey);
  
  console.log("💰 Final Balances:");
  console.log(`   Player 1: ${balance1.balance / 1_000_000} USDC`);
  console.log(`   Player 2: ${balance2.balance / 1_000_000} USDC\n`);
  
  console.log("=".repeat(80));
  console.log("✅ MENTAL POKER GAME COMPLETED SUCCESSFULLY");
  console.log("=".repeat(80) + "\n");
  
  console.log("✨ What Just Happened:");
  console.log("   • Both players shuffled the deck independently");
  console.log("   • Cards remained encrypted until revealed");
  console.log("   • No player could see opponent's cards");
  console.log("   • No player could predict which cards would come");
  console.log("   • All reveals verified cryptographically on-chain");
  console.log("   • Merkle proofs ensured deck integrity");
  console.log("   • Completely trustless - no dealer needed!\n");
}

if (require.main === module) {
  main()
    .then(() => process.exit(0))
    .catch((error) => {
      console.error("\n❌ Error:", error);
      console.error("\n💡 Make sure:");
      console.error("   • Solana test validator is running");
      console.error("   • Program is deployed (anchor build && anchor deploy)");
      console.error("   • You have sufficient SOL for transactions\n");
      process.exit(1);
    });
}

export { main };

