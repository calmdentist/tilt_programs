#!/bin/bash

set -e  # Exit on error

echo "============================================"
echo "🎴 ZK Poker - Cryptography Demo"
echo "============================================"
echo ""

# Step 1: Install dependencies
echo "📦 Installing dependencies..."
bun install

echo ""
echo "============================================"
echo "🎮 Running Full Game Example"
echo "   (Paillier + ZK-SNARKs, No Smart Contract)"
echo "============================================"
echo ""

# Run the crypto-only full game example
bunx tsx app/full-game-example.ts

echo ""
echo "============================================"
echo "✅ Demo Complete!"
echo "============================================"
echo ""
echo "📊 This demo showed:"
echo "   • Paillier key generation (2048-bit)"
echo "   • Deck encryption & shuffling"
echo "   • ZK-SNARK proof generation (3 types)"
echo "   • Complete poker hand (pre-flop → showdown)"
echo "   • Performance metrics (~3-4 seconds total)"
echo ""
echo "📖 For more examples:"
echo "   npx ts-node app/paillier-mental-poker.ts"
echo ""
echo "📚 Documentation:"
echo "   See CRYPTO_EXAMPLES.md for details"
echo "   See PERFORMANCE_RESULTS.md for benchmarks"
echo "   See QUICKSTART_CRYPTO.md for quick start"
echo ""

