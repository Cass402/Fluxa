# Using node:22 as the base image for the frontend
# This image is used to build the frontend application
FROM node:22 as frontend-base

# Set the working directory
WORKDIR /app/frontend
# Copy package.json and package-lock.json
COPY package.json package-lock.json ./
# Install dependencies
RUN npm install
# Copy the rest of the frontend files
COPY frontend/ ./

FROM rust:1.85.1 as rust-base

# Install Solana CLI
RUN sh -c "$(curl -sSfL https://release.anza.xyz/v2.1.0/install)" && \
    export PATH="/root/.local/share/solana/install/active_release/bin:$PATH"

# Set the working directory
WORKDIR /app/programs

# Install Anchor
RUN cargo install --git https://github.com/coral-xyz/anchor avm --force && \
    avm install 0.31.0 && \
    avm use 0.31.0

# Copy the rest of the program files
COPY programs/ ./


# Development image with all tools
# From the rust-base image
FROM rust-base as development

# Copy the frontend build from the frontend-base image
COPY --from=frontend-base /app/frontend /app/frontend

# Set the working directory
WORKDIR /app

# Copy the rest of the files
COPY scripts/ ./scripts/
COPY tests/ ./tests/
COPY .env ./

# Initialize the local validator
RUN solana-keygen new --no-bip39-passphrase -o /app/test-keypair.json && \
    solana config set -keypair /app/test-keypair.json

# Set entrypoint
COPY docker-entrypoint.sh /
RUN chmod +x /docker-entrypoint.sh
ENTRYPOINT ["/docker-entrypoint.sh"]