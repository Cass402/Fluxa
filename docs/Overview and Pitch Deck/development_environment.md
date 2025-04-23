# Fluxa Development Environment Guide

This document describes how to set up and work with the Fluxa development environment.

## Prerequisites

- [Docker](https://docs.docker.com/get-docker/) and Docker Compose
- [Git](https://git-scm.com/downloads)
- [VS Code](https://code.visualstudio.com/) (recommended)
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools) (version 2.1.0)
- [Anchor CLI](https://www.anchor-lang.com/docs/installation) (version 0.31.0)

## Initial Setup

1. **Clone the Repository**

   ```bash
   git clone https://github.com/your-username/fluxa.git
   cd fluxa
   ```

2. **Initialize Git Hooks**

   ```bash
   make init-hooks
   ```

3. **Start the Development Environment**

   ```bash
   make up
   ```

4. **Install VS Code Extensions**
   Open the project in VS Code and install the recommended extensions when prompted.

## Common Development Tasks

All common tasks are available as Make commands. Here are the most frequently used:

- `make up` - Start the development environment
- `make down` - Stop the development environment
- `make build` - Rebuild the Docker images
- `make test` - Run all tests
- `make format` - Format all code
- `make lint` - Run linters on all code
- `make check` - Run both formatting and linting checks
- `make dev-shell` - Get a shell inside the development container

## Project Structure

- `/programs/src/` - Solana programs written in Rust using Anchor
  - `/programs/src/amm_core/` - Core AMM functionality
  - `/programs/src/impermanent_loss/` - Impermanent loss mitigation
  - `/programs/src/order_book/` - Order book functionality
  - `/programs/src/yield_optimization/` - Yield optimization strategies
- `/frontend/` - Frontend application
- `/tests/` - Integration tests
- `/scripts/` - Utility scripts
- `/docs/` - Project documentation

## Development Workflow

1. **Creating a New Feature**

   - Create a feature branch: `git checkout -b feature/your-feature-name`
   - Implement your changes
   - Run tests: `make test`
   - Ensure code quality: `make check`
   - Commit and push your changes

2. **Running Tests**

   - Unit tests: `make test`
   - Test a specific program: `docker-compose exec fluxa-dev bash -c "cd /app && anchor test -- --program amm_core"`

3. **Debugging**
   - Check logs: `make logs`
   - Access the dev shell: `make dev-shell`

## CI/CD Pipeline

Our project uses GitHub Actions for continuous integration and deployment:

1. **Pull Request Pipeline**

   - Automatically runs on every PR
   - Builds the project
   - Runs formatting checks and linting
   - Executes all tests

2. **Release Pipeline**
   - Triggered when pushing a tag starting with `v` (e.g., `v1.0.0`)
   - Builds and tests the project
   - Creates a GitHub release with program artifacts
   - (Future) Will deploy to Solana Mainnet when ready

## Deployment

### Local/Development

To deploy to localnet for testing:

```bash
make deploy
```

### Testnet/Devnet

To deploy to Solana Devnet:

```bash
# Configure your Solana CLI first
solana config set --url devnet
solana-keygen new --outfile ~/.config/solana/id.json  # If you don't have a keypair
solana airdrop 5  # Request test SOL

# Then deploy
docker-compose exec fluxa-dev bash -c "cd /app && anchor deploy --provider.cluster devnet"
```

## Troubleshooting

### Common Issues

1. **Docker container won't start**

   - Ensure Docker is running: `docker info`
   - Check for port conflicts: `docker ps -a`
   - Try rebuilding: `make down && make build && make up`

2. **Build errors**

   - Try cleaning the build: `make clean`
   - Check logs: `make logs`

3. **Git hooks not running**
   - Verify hooks are initialized: `ls -la .git/hooks`
   - Reinitialize if needed: `make init-hooks`

### Getting Help

If you encounter issues not covered here, please:

1. Check existing GitHub issues
2. Create a new issue with details about the problem
3. Reach out to the core team on Discord
