#!/bin/bash
set -e

# Create directory for wallets if it doesn't exist
mkdir -p ./wallets

# Generate test wallets
echo "Generating test wallets..."
solana-keygen new -o ./wallets/wallet1.json --no-bip39-passphrase
solana-keygen new -o ./wallets/wallet2.json --no-bip39-passphrase

# Display public keys
PUBKEY1=$(solana-keygen pubkey ./wallets/wallet1.json)
PUBKEY2=$(solana-keygen pubkey ./wallets/wallet2.json)
echo "Wallet 1: $PUBKEY1"
echo "Wallet 2: $PUBKEY2"

# Airdrop SOL
echo "Airdropping SOL to test wallets..."
solana airdrop 100 $PUBKEY1 --url localhost
solana airdrop 100 $PUBKEY2 --url localhost

echo "Test accounts setup complete!"