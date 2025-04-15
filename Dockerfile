# Using node:22 as the base image for the frontend
# This image is used to build the frontend application
FROM node:22 AS frontend-base

# Set the working directory
WORKDIR /app/frontend
# Copy package.json and package-lock.json
COPY frontend/package.json frontend/package-lock.json ./
# Install dependencies
RUN npm install
# Copy the rest of the frontend files
COPY frontend/ ./

FROM rust:1.85.1 AS rust-base

# Set the working directory for rust-base
WORKDIR /app/programs

# Copy the program files
COPY programs/ ./


# Development image with all tools
# From the rust base image
FROM rust:1.85.1 AS development

# Install Solana CLI in the development stage
RUN sh -c "$(curl -sSfL https://release.solana.com/v2.1.0/install)" && \
    echo 'export PATH="/root/.local/share/solana/install/active_release/bin:$PATH"' >> ~/.bashrc

# Ensure the PATH is available in all subsequent RUN commands
ENV PATH="/root/.local/share/solana/install/active_release/bin:$PATH"

# Install Anchor using the prebuilt NPM package instead of compiling from source
RUN apt-get update && apt-get install -y nodejs npm && \
    npm install -g @coral-xyz/anchor-cli@^0.31.0 && \
    anchor --version

# Copy the frontend build from the frontend-base image
COPY --from=frontend-base /app/frontend /app/frontend

# Copy the rust programs from the rust-base image
COPY --from=rust-base /app/programs /app/programs

# Set the working directory
WORKDIR /app

# Copy the rest of the files
COPY scripts/ ./scripts/
COPY tests/ ./tests/
COPY .env ./

# Create keypair and configure Solana CLI
RUN solana-keygen new --no-bip39-passphrase -o /app/test-keypair.json && \
    solana config set -keypair /app/test-keypair.json

# Set entrypoint
COPY docker-entry_point.sh /
RUN chmod +x /docker-entry_point.sh
ENTRYPOINT ["/docker-entry_point.sh"]