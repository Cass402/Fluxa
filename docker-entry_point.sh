#!/bin/bash
set -e

# Start Solana test validator in the background
echo "Starting Solana test validator..."
solana-test-validator --reset --quiet &
VALIDATOR_PID=$!

# Wait for validator to be ready
sleep 5
echo "Validator running with PID: $VALIDATOR_PID"

# Set URL to local validator
solana config set --url http://localhost:8899

# Execute command passed to docker run
exec "$@"