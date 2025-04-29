# GitHub Environments Setup Guide

This document outlines the GitHub environments required for the Fluxa CI/CD pipeline.

## Required Environments

The Fluxa project requires three environments to be configured in GitHub:

1. **Development (devnet)**
   - For development and early testing
   - No approval requirements
2. **Staging (testnet)**
   - For pre-production testing
   - Optional approval requirements
3. **Production (mainnet)**
   - For production deployments
   - Strict approval requirements

## Environment Setup Instructions

### 1. Access Repository Settings

1. Navigate to your GitHub repository
2. Click on "Settings"
3. In the left sidebar, click on "Environments"

### 2. Create Development Environment

1. Click "New environment"
2. Name: `development`
3. No protection rules needed
4. Add the following secrets:
   - `SOLANA_DEVNET_PRIVATE_KEY`: The keypair JSON for devnet deployments
   - `SLACK_WEBHOOK`: Webhook URL for notifications (development channel)

### 3. Create Staging Environment

1. Click "New environment"
2. Name: `staging`
3. Protection rules:
   - Optional: Enable "Required reviewers" and add reviewers who can approve testnet deployments
   - Deployment branches: `staging`, `main`
4. Add the following secrets:
   - `SOLANA_TESTNET_PRIVATE_KEY`: The keypair JSON for testnet deployments
   - `SLACK_WEBHOOK`: Webhook URL for notifications (staging channel)

### 4. Create Production Environment

1. Click "New environment"
2. Name: `production`
3. Protection rules:
   - Enable "Required reviewers" and add reviewers who can approve mainnet deployments
   - Set "Wait timer" to 15 minutes to allow time for review
   - Deployment branches: `main` only
4. Add the following secrets:
   - `SOLANA_MAINNET_PRIVATE_KEY`: The keypair JSON for mainnet deployments
   - `SLACK_WEBHOOK`: Webhook URL for notifications (production channel)
   - `FIREBASE_SERVICE_ACCOUNT`: For frontend deployment

## Secrets Management

- Ensure all Solana private keys are properly secured and never committed to the repository
- Rotate keys on a regular schedule
- Limit access to production environment secrets to essential team members only
- Document key rotation procedures and emergency access protocols

## Environment Variables

Each environment has its own set of environment variables that can be referenced in workflows:

```yaml
env:
  REACT_APP_ENVIRONMENT: ${{ github.ref == 'refs/heads/main' && 'production' || 'development' }}
  REACT_APP_API_URL: ${{ github.ref == 'refs/heads/main' && 'https://api.fluxa.io' || 'https://dev-api.fluxa.io' }}
```

## Branch Protection

In addition to environment protection, set up branch protection rules:

1. `main` branch:

   - Require pull request reviews before merging
   - Require status checks to pass (CI/CD workflow)
   - Do not allow bypassing the above settings

2. `staging` branch:
   - Require status checks to pass
   - Allow administrators to bypass
