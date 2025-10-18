# Tilt Poker Protocol - Deployment Guide

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.70+
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools) 1.16+
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) 0.28+
- [Node.js](https://nodejs.org/) 18+
- [Yarn](https://yarnpkg.com/) or npm

## Installation

```bash
# Clone the repository
git clone <your-repo>
cd tilt_programs

# Install dependencies
yarn install

# Build the program
anchor build
```

## Local Development

### 1. Start Local Validator

```bash
# Start Solana test validator
solana-test-validator
```

Keep this running in a separate terminal.

### 2. Configure Solana CLI

```bash
# Set to localhost
solana config set --url localhost

# Check configuration
solana config get

# Create a wallet if you don't have one
solana-keygen new

# Airdrop SOL for testing
solana airdrop 2
```

### 3. Build and Deploy

```bash
# Build the program
anchor build

# Deploy to local validator
anchor deploy

# Get the program ID
solana address -k target/deploy/tilt_programs-keypair.json
```

### 4. Update Program ID

Update the program ID in:
- `Anchor.toml` (programs.localnet section)
- `lib.rs` (declare_id! macro)
- `tests/` files if needed

Then rebuild:
```bash
anchor build
anchor deploy
```

### 5. Run Tests

```bash
# Run all tests
anchor test

# Run specific test file
anchor test tests/poker-game.ts

# Skip deploy (if already deployed)
anchor test --skip-deploy
```

### 6. Run Example Game

```bash
# Run example game script
yarn tsx app/example-game.ts
```

## Devnet Deployment

### 1. Configure for Devnet

```bash
# Set cluster to devnet
solana config set --url devnet

# Airdrop SOL on devnet (limited)
solana airdrop 2

# Or use a devnet faucet:
# https://faucet.solana.com/
```

### 2. Update Anchor.toml

```toml
[provider]
cluster = "Devnet"
wallet = "~/.config/solana/id.json"

[programs.devnet]
tilt_programs = "YourProgramIdHere"
```

### 3. Deploy to Devnet

```bash
# Build
anchor build

# Deploy
anchor deploy --provider.cluster devnet

# Verify deployment
solana program show <PROGRAM_ID> --url devnet
```

### 4. Test on Devnet

```bash
# Run tests on devnet
anchor test --provider.cluster devnet
```

## Mainnet Deployment

⚠️ **WARNING**: Mainnet deployment involves real funds. Ensure thorough testing on devnet first.

### 1. Security Checklist

- [ ] Complete security audit
- [ ] Extensive testing on devnet
- [ ] Verify all error handling
- [ ] Test with real user scenarios
- [ ] Set up monitoring
- [ ] Prepare incident response plan
- [ ] Have upgrade strategy ready

### 2. Prepare Mainnet Wallet

```bash
# Create mainnet wallet (or use existing)
solana-keygen new -o ~/mainnet-deploy-keypair.json

# Fund with SOL (deployment costs ~5-10 SOL)
# Transfer from exchange or another wallet

# Check balance
solana balance ~/mainnet-deploy-keypair.json --url mainnet-beta
```

### 3. Configure for Mainnet

```bash
# Set to mainnet
solana config set --url mainnet-beta
solana config set --keypair ~/mainnet-deploy-keypair.json
```

Update `Anchor.toml`:
```toml
[provider]
cluster = "Mainnet"
wallet = "~/mainnet-deploy-keypair.json"

[programs.mainnet]
tilt_programs = "YourProgramIdHere"
```

### 4. Deploy to Mainnet

```bash
# Final build
anchor build

# Deploy (THIS COSTS REAL SOL)
anchor deploy --provider.cluster mainnet-beta

# Verify
solana program show <PROGRAM_ID> --url mainnet-beta
```

### 5. Post-Deployment

- Verify program is upgradeable (or locked if intended)
- Set up monitoring for transactions
- Create public documentation
- Announce to community

## Program Upgrades

### Upgrade Process

```bash
# Make changes to code
# Update version in Cargo.toml

# Build new version
anchor build

# Upgrade (costs SOL)
anchor upgrade target/deploy/tilt_programs.so --program-id <PROGRAM_ID>

# Verify upgrade
solana program show <PROGRAM_ID>
```

### Making Program Immutable

Once fully tested and audited, you can make the program immutable:

```bash
# Remove upgrade authority (IRREVERSIBLE!)
solana program set-upgrade-authority <PROGRAM_ID> --final
```

## Monitoring & Maintenance

### Monitor Transactions

```bash
# Watch program logs
solana logs <PROGRAM_ID>

# Get program info
solana program show <PROGRAM_ID>

# Check account data
solana account <ACCOUNT_PUBKEY>
```

### Using Explorers

- **Solana Explorer**: https://explorer.solana.com/
- **Solscan**: https://solscan.io/
- **Solana Beach**: https://solanabeach.io/

### Setting Up Alerts

Consider using services like:
- [Dialect](https://www.dialect.to/) - Solana notifications
- [Helius](https://helius.xyz/) - Enhanced RPC with webhooks
- Custom indexer with GraphQL

## Client Integration

### Frontend Integration

```typescript
import { createPokerClient } from './app/poker-client';
import { AnchorProvider } from '@coral-xyz/anchor';
import { useAnchorWallet } from '@solana/wallet-adapter-react';

function PokerApp() {
  const wallet = useAnchorWallet();
  const connection = new Connection(clusterApiUrl('mainnet-beta'));
  const provider = new AnchorProvider(connection, wallet, {});
  
  const client = createPokerClient(program, provider);
  
  // Use client methods...
  const { gameStatePDA } = await client.createGame(
    playerKeypair,
    stakeAmount
  );
}
```

### Backend/Bot Integration

```typescript
import { Keypair } from '@solana/web3.js';
import { createPokerClient } from './app/poker-client';

// Load bot wallet
const botKeypair = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync('bot-wallet.json')))
);

// Create client
const client = createPokerClient(program, provider);

// Automated gameplay
async function playHand(gameStatePDA) {
  const gameState = await client.getGameState(gameStatePDA);
  
  // Implement strategy
  if (shouldFold(gameState)) {
    await client.playerAction(botKeypair, gameStatePDA, PlayerAction.Fold);
  } else if (shouldRaise(gameState)) {
    await client.playerAction(
      botKeypair, 
      gameStatePDA, 
      PlayerAction.Raise,
      raiseAmount
    );
  }
}
```

## Troubleshooting

### Common Issues

#### Build Errors

```bash
# Clear and rebuild
anchor clean
anchor build

# Update dependencies
cargo update
```

#### Deployment Fails

```bash
# Increase max program size in Anchor.toml
[programs.localnet]
tilt_programs = "PROGRAM_ID"

# Or use:
solana program deploy --max-len 200000 target/deploy/tilt_programs.so
```

#### Transaction Size Exceeded

- Break large instructions into smaller ones
- Use compute unit limits
- Optimize account sizes

#### Timeout Errors

```bash
# Increase timeout in Anchor.toml
[provider]
timeout = 60000  # milliseconds
```

### Getting Help

- Check the [Anchor Discord](https://discord.gg/anchor)
- Review [Solana Stack Exchange](https://solana.stackexchange.com/)
- Open an issue on GitHub

## Performance Optimization

### RPC Optimization

```typescript
// Use commitment levels appropriately
const connection = new Connection(RPC_URL, {
  commitment: 'confirmed',  // Faster than 'finalized'
});

// Batch requests
const accounts = await connection.getMultipleAccountsInfo([...pubkeys]);
```

### Transaction Optimization

```typescript
// Use versioned transactions for better efficiency
import { VersionedTransaction } from '@solana/web3.js';

// Set compute unit limit and price
const computeUnitIx = ComputeBudgetProgram.setComputeUnitLimit({
  units: 200000,
});
const priorityFeeIx = ComputeBudgetProgram.setComputeUnitPrice({
  microLamports: 1,
});
```

### Account Compression

For storing many game states, consider:
- [State Compression](https://docs.solana.com/developing/on-chain-programs/state-compression)
- [Merkle Trees](https://www.soldev.app/course/compressed-nfts)
- Off-chain storage with on-chain verification

## Costs Estimation

### Program Deployment

- **Devnet**: Free (test SOL)
- **Mainnet**: ~5-10 SOL (depends on program size)

### Transaction Costs

Per game (estimated):
- Initialize player: ~0.00001 SOL
- Create game: ~0.00001 SOL
- Join game: ~0.00001 SOL
- Each action: ~0.00001 SOL
- **Total per game**: ~0.0001 SOL

### Account Rent

- PlayerAccount: Rent-exempt (~0.002 SOL one-time)
- GameState: Rent-exempt (~0.01 SOL per game, reclaimable)

## Next Steps

1. ✅ Deploy to devnet
2. ✅ Test thoroughly
3. ✅ Build frontend
4. ✅ Run beta testing
5. ✅ Security audit
6. ✅ Deploy to mainnet
7. ✅ Monitor and iterate

## Resources

- [Anchor Docs](https://www.anchor-lang.com/)
- [Solana Cookbook](https://solanacookbook.com/)
- [Solana Program Library](https://spl.solana.com/)
- [This Repository](../README.md)

## Support

For issues or questions:
- Open an issue on GitHub
- Join our Discord community
- Check the documentation

---

**Remember**: Always test thoroughly on devnet before mainnet deployment!

