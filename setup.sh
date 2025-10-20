#!/bin/bash

set -e  # Exit on error

echo "============================================"
echo "🎴 ZK Poker Setup Script"
echo "============================================"
echo ""

# Step 1: Kill existing validators
echo "🛑 Killing any existing Solana test validators..."
pkill -f "solana-test-validator" || true
sleep 2

# Step 2: Install dependencies
echo ""
echo "📦 Installing dependencies..."
bun install

# Step 3: Build the program
echo ""
echo "🔨 Building Anchor program..."
anchor build

# Step 4: Copy deterministic program keypair to target/deploy
echo ""
echo "🔑 Copying program keypair to target/deploy..."
mkdir -p target/deploy
cp keypairs/zkpoker-keypair.json target/deploy/zkpoker-keypair.json

# Step 5: Start test validator
echo ""
echo "🚀 Starting Solana test validator..."
solana-test-validator --reset > validator.log 2>&1 &
VALIDATOR_PID=$!

# Wait for validator to be ready
echo "⏳ Waiting for validator to be ready..."
max_attempts=30
attempt=0

while ! solana cluster-version > /dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ $attempt -ge $max_attempts ]; then
        echo "❌ Validator failed to start after $max_attempts seconds"
        kill $VALIDATOR_PID 2>/dev/null || true
        exit 1
    fi
    sleep 1
done

echo "✅ Validator is ready!"

# Give validator a moment to stabilize
sleep 2

# Step 6: Deploy the program
echo ""
echo "📤 Deploying program to test validator..."
anchor deploy

# Step 7: Run the full game example
echo ""
echo "============================================"
echo "🎮 Running Full Game Example"
echo "============================================"
echo ""

bun run app/full-game-example.ts

echo ""
echo "============================================"
echo "✅ Setup Complete!"
echo "============================================"
echo ""
echo "ℹ️  Validator is still running (PID: $VALIDATOR_PID)"
echo "   To stop it: pkill -f solana-test-validator"
echo "   Or just run: ./setup.sh again (it auto-restarts)"
echo ""

