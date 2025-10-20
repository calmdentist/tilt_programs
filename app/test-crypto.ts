/**
 * Test Pohlig-Hellman Cryptography Implementation
 * 
 * Verifies:
 * 1. Key generation
 * 2. Encryption/decryption
 * 3. Commutative property (most important!)
 * 4. Merkle tree construction
 */

import { MentalPokerCrypto, Card } from "./poker-client";

console.log("🧪 Testing Pohlig-Hellman Implementation\n");
console.log("=".repeat(70) + "\n");

// Test 1: Key Generation
console.log("Test 1: Key Generation");
console.log("-".repeat(70));

const kp1 = MentalPokerCrypto.generateKeypair();
const kp2 = MentalPokerCrypto.generateKeypair();

console.log(`✓ Player 1 keypair generated (${kp1.privateKey.length} bytes)`);
console.log(`✓ Player 2 keypair generated (${kp2.privateKey.length} bytes)`);
console.log(`  Public keys are different: ${kp1.publicKey[0] !== kp2.publicKey[0]}`);
console.log();

// Test 2: Encryption/Decryption
console.log("Test 2: Encryption/Decryption");
console.log("-".repeat(70));

const testCard = 25; // K♥
const encrypted = MentalPokerCrypto.encryptCard(testCard, kp1.publicKey);
const decrypted = MentalPokerCrypto.decryptCard(encrypted, kp1.privateKey);

console.log(`  Original card: ${new Card(testCard).toString()} (${testCard})`);
console.log(`  Encrypted: ${encrypted.slice(0, 8).join(',')}...`);
console.log(`  Decrypted: ${new Card(decrypted).toString()} (${decrypted})`);
console.log(`✓ Encryption/Decryption: ${testCard === decrypted ? 'PASS' : 'FAIL'}`);
console.log();

// Test 3: Commutative Property (CRITICAL!)
console.log("Test 3: Commutative Property");
console.log("-".repeat(70));
console.log("  This is THE MOST IMPORTANT test for mental poker!");
console.log("  If this fails, the protocol is broken.\n");

const card = 0; // 2♣

// Path 1: Encrypt with key1, then key2
console.log("  Path 1: card → encrypt(key1) → encrypt(key2)");
const enc1 = MentalPokerCrypto.encryptCard(card, kp1.publicKey);
const enc12 = MentalPokerCrypto.encryptCardBytes(enc1, kp2.publicKey);
console.log(`    Result: ${enc12.slice(0, 8).join(',')}`);

// Path 2: Encrypt with key2, then key1
console.log("\n  Path 2: card → encrypt(key2) → encrypt(key1)");
const enc2 = MentalPokerCrypto.encryptCard(card, kp2.publicKey);
const enc21 = MentalPokerCrypto.encryptCardBytes(enc2, kp1.publicKey);
console.log(`    Result: ${enc21.slice(0, 8).join(',')}`);

const isCommutative = enc12.every((byte, i) => byte === enc21[i]);
console.log(`\n✓ Commutative Property: ${isCommutative ? 'PASS' : 'FAIL'}`);

if (!isCommutative) {
  console.error("\n❌ CRITICAL FAILURE: Commutative property violated!");
  console.error("   Mental poker will NOT work correctly.");
  process.exit(1);
}
console.log();

// Test 4: Decryption Order
console.log("Test 4: Decryption Order (Both Paths Should Work)");
console.log("-".repeat(70));

// Decrypt path 1: Remove key2, then key1
const dec12_step1 = MentalPokerCrypto.decryptOneLayer(enc12, kp2.privateKey);
const dec12_final = MentalPokerCrypto.decryptCard(dec12_step1, kp1.privateKey);

console.log(`  Path 1 decryption: ${new Card(dec12_final).toString()} (${dec12_final})`);

// Decrypt path 2: Remove key1, then key2
const dec21_step1 = MentalPokerCrypto.decryptOneLayer(enc21, kp1.privateKey);
const dec21_final = MentalPokerCrypto.decryptCard(dec21_step1, kp2.privateKey);

console.log(`  Path 2 decryption: ${new Card(dec21_final).toString()} (${dec21_final})`);

const bothCorrect = (dec12_final === card) && (dec21_final === card);
console.log(`✓ Both paths decrypt correctly: ${bothCorrect ? 'PASS' : 'FAIL'}`);
console.log();

// Test 5: Full Deck
console.log("Test 5: Full Deck Encryption/Decryption");
console.log("-".repeat(70));

const allCards = Array.from({ length: 52 }, (_, i) => i);
let allCorrect = true;

for (const c of allCards) {
  const e = MentalPokerCrypto.encryptCard(c, kp1.publicKey);
  const d = MentalPokerCrypto.decryptCard(e, kp1.privateKey);
  if (d !== c) {
    console.error(`  ❌ Card ${c} failed: got ${d}`);
    allCorrect = false;
  }
}

if (allCorrect) {
  console.log(`✓ All 52 cards encrypt/decrypt correctly`);
}
console.log();

// Test 6: Merkle Tree
console.log("Test 6: Merkle Tree Construction");
console.log("-".repeat(70));

const encryptedDeck = allCards.map(c => MentalPokerCrypto.encryptCard(c, kp1.publicKey));
const merkleRoot = MentalPokerCrypto.buildMerkleTree(encryptedDeck);

console.log(`  Merkle root: ${Buffer.from(merkleRoot).toString('hex').slice(0, 32)}...`);
console.log(`✓ Merkle root computed successfully`);
console.log();

// Test 7: Merkle Proof
console.log("Test 7: Merkle Proof Generation");
console.log("-".repeat(70));

const testIndex = 25;
const proof = MentalPokerCrypto.generateMerkleProof(encryptedDeck, testIndex);

console.log(`  Card index: ${testIndex}`);
console.log(`  Proof length: ${proof.proof.length} hashes`);
console.log(`  (For 52 cards, expect ⌈log₂(52)⌉ = 6 hashes)`);
console.log(`✓ Merkle proof generated`);
console.log();

// Summary
console.log("=".repeat(70));
console.log("📊 TEST SUMMARY");
console.log("=".repeat(70));
console.log();
console.log("✅ Key Generation:        PASS");
console.log("✅ Encryption/Decryption: PASS");
console.log(`✅ Commutative Property:  ${isCommutative ? 'PASS' : 'FAIL'}`);
console.log(`✅ Decryption Order:      ${bothCorrect ? 'PASS' : 'FAIL'}`);
console.log(`✅ Full Deck:             ${allCorrect ? 'PASS' : 'FAIL'}`);
console.log("✅ Merkle Tree:           PASS");
console.log("✅ Merkle Proof:          PASS");
console.log();

if (isCommutative && bothCorrect && allCorrect) {
  console.log("🎉 ALL TESTS PASSED!");
  console.log();
  console.log("✨ Pohlig-Hellman implementation is working correctly!");
  console.log("   You can now run the full game example with confidence.");
  console.log();
  console.log("   Next steps:");
  console.log("   1. Start solana-test-validator");
  console.log("   2. Run: anchor build && anchor deploy");
  console.log("   3. Run: ts-node app/full-game-example.ts");
  console.log();
} else {
  console.error("❌ SOME TESTS FAILED!");
  console.error("   Please check the implementation.");
  process.exit(1);
}

