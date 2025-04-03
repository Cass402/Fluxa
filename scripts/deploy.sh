#!/bin/bash
set -e

echo "Building and deploying Fluxa programs..."

# Navigate to anchor project
cd /app/programs

# Build the programs
echo "Building programs..."
anchor build

# Deploy to localnet
echo "Deploying to localnet..."
anchor deploy

echo "Deployment complete!"