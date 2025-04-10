# Fluxa Docker Development Environment

This document explains how to use the Docker-based development environment for Fluxa.

## Prerequisites

- Docker
- Docker Compose
- Git
- VS Code with Remote-Containers extension (optional but recommended)

## Quick Start

1. Clone the repository:

   ```bash
   git clone https://github.com/Cass402/Fluxa.git
   cd Fluxa
   ```

2. Start the development environment:

   ```bash
   make up
   ```

   This will:

   - Build the Docker containers
   - Start a local Solana validator
   - Set up the development environment

3. Set up test accounts:

   ```bash
   make setup-test-accounts
   ```

4. Access the development shell:

   ```bash
   make shell
   ```

   In VS Code, you can use the "Remote-Containers: Open Folder in Container" command to connect to the development container.

## Common Tasks

- **Build the application**:
  ```bash
  make build
  ```
- **Run tests**:
  ```bash
  make test
  ```
- **Deploy to localnet**:
  ```bash
  make deploy
  ```
- **Clean build artifacts**:
  ```bash
  make clean
  ```
- **Stop the environment**:
  ```bash
  make down
  ```

## Container Structure

- **fluxa-dev**: Main development container with Rust, Solana, and Node.js
- **fluxa-frontend**: Optional separate container for frontend-only work

## Folder Structure

- `/app/programs`: Solana programs written with Anchor
- `/app/frontend`: Next.js frontend application
- `/app/tests`: Test suites
- `/app/scripts`: Utility scripts

## Troubleshooting

- If the validator fails to start:
  ```bash
  make down
  make up
  ```
- If dependencies are out of date:
  ```bash
  make build
  make up
  ```
- To reset the local blockchain:
  ```bash
  docker-compose exec fluxa-dev solana-test-validator --reset
  ```

## Benefits of This Setup

1. **Consistent Environment**: Everyone runs exactly the same versions of all tools.
2. **Isolated Development**: Doesn't pollute your local machine with dependencies.
3. **Quick Onboarding**: New team members can start coding in minutes.
4. **Easy Testing**: Run tests in the same environment as development.
5. **VS Code Integration**: Seamless development experience with Remote-Containers extension.

This Docker-based approach will eliminate "works on my machine" problems during the hackathon and save valuable time that would otherwise be spent debugging environment issues. The team can focus entirely on implementation rather than configuration.
