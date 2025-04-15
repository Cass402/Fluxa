# Fluxa

**Fluidity meets function**

**Hybrid Adaptive AMM & Personalized Yield Optimizer on Solana**

Fluxa is a next-generation decentralized finance (DeFi) protocol built on Solana. It combines the power of concentrated liquidity, dynamic impermanent loss mitigation, and personalized yield strategies to deliver a secure, efficient, and user-friendly liquidity provisioning experience. Fluxa is designed to maximize capital efficiency, reduce risk, and make advanced DeFi accessible to both novice and experienced users.

---

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Architecture](#architecture)
- [Testing](#testing)
- [Development Setup](#development-setup)
- [Additional Documentation](#additional-documentation)
- [Contributing](#contributing)
- [Roadmap](#roadmap)
- [License](#license)

---

## Overview

Fluxa is a Hybrid Adaptive AMM that uniquely integrates:

- **Concentrated Liquidity** (similar to Uniswap v3) – allowing liquidity providers (LPs) to define custom price ranges.
- **Integrated Order Book** (Serum-style) – enabling limit order placement directly on liquidity pools.
- **Dynamic Liquidity Curves** – auto-adjusting to market volatility to minimize impermanent loss.
- **Personalized Yield Optimization** – offering tailored yield strategies (Conservative, Balanced, Aggressive) based on user-selected risk profiles.
- **Optimized UX & Onboarding** – featuring a clean interface with integrated educational resources and a seamless fiat on-ramp.

Fluxa leverages Solana’s parallel execution model, providing high throughput and low latency, making it a robust solution for today's fast-paced DeFi landscape.

---

## Features

- **Hybrid Adaptive AMM Model:**  
   Combines concentrated liquidity and an integrated order book for enhanced trading precision and capital efficiency.
- **Impermanent Loss Mitigation Protocol:**  
   Dynamic rebalancing and an insurance fund help mitigate risks associated with volatile market conditions.
- **Personalized Yield Optimization:**  
   Users can select risk profiles to receive customized yield strategies and real-time performance analytics.
- **User-Friendly Interface:**  
   An intuitive dashboard with gamified scorecards, real-time updates, and guided onboarding ensures a seamless user experience.
- **Solana Optimization:**  
   Fully optimized to leverage Solana's parallel transaction execution, ensuring rapid, low-cost transactions.

---

## Architecture

Fluxa is architected as a collection of modular on-chain programs (built using the Anchor framework) that interact via Solana’s cross-program invocations (CPIs). The key modules include:

- **AMM Core Module:**  
   Manages liquidity pools, fee accrual, and pricing based on custom liquidity ranges.
- **Order Book Module:**  
   Enables users to place and manage limit orders, integrating Serum-style order matching within the AMM.
- **Impermanent Loss Mitigation Module:**  
   Dynamically adjusts liquidity curves and triggers rebalancing to protect LP funds.
- **Personalized Yield Optimization Module:**  
   Adapts yield strategies based on user-selected risk profiles, adjusting compounding and rebalancing parameters in real time.
- **Insurance Fund Module:**  
   Collects a portion of trading fees to cover significant IL events and maintain liquidity stability.

External integrations include partnerships with protocols such as Marinade, Solend, and Jupiter Aggregator to further enhance liquidity and yield options.

For an overview of the architecture, please refer to the [Architecture Document](docs/architecture.md).

For a detailed technical design, please refer to the [Detailed Technical Design Document](docs/detailed-technical-design.md).

---

## Testing

Fluxa undergoes rigorous testing to ensure security and functionality:

- **Unit Testing:**  
   Each function and module is tested individually.
- **Integration Testing:**  
   Simulated interactions across modules (e.g., liquidity provision, order matching, IL mitigation) are verified on a local validator.
- **Fuzz Testing:**  
   Randomized input tests ensure robustness against edge cases.
- **Property-Based Testing:**  
   Key invariants (such as liquidity conservation and fee distribution) are verified across diverse market scenarios.

For more details, check the [Security Testing Checklist](docs/securityTestingChecklist.md) and the [Test Plan + Coverage Report Document](docs/testPlan_coverageReport.md).

---

## Development Setup

Fluxa uses a Docker-based development environment to ensure consistent setup across all developer machines.

### Prerequisites

- Docker
- Docker Compose
- Git
- VS Code with Remote-Containers extension (optional but recommended)

### Getting Started

For detailed instructions on setting up the development environment, please refer to the [Docker Setup Guide](Fluxa_Docker_Setup.md) and [Development Environment Setup Guide](/docs/development_environment.md).

Quick start:

```bash
# Clone the repository
git clone https://github.com/Cass402/Fluxa.git
cd Fluxa

# Start the development environment
make up

# Set up test accounts
make setup-test-accounts

# Access the development shell
make shell
```

This Docker-based setup eliminates "works on my machine" problems and allows you to start coding in minutes.

---

## Additional Documentation

### Project and Technical Documentation

- [Project Overview and Executive Summary](docs/projectOverview_and_executiveSummary.md)
- [Requirements Document](docs/requirements.md)
- [Architecture Document](docs/architecture.md)
- [Detailed Technical Design](docs/detailedTechnicalDesign.md)
- [Impermanent Loss Mitigation Deep Dive](docs/ILMitigation_DeepDive.md)
- [Implementation Timeline](docs/implementationTimeline.md)

### Security and Testing

- [Threat Model and Risk Assessment](docs/threatModel_and_riskAssessment.md)
- [Security Testing Checklist](docs/securityTestingChecklist.md)
- [Test Plan and Coverage Report](docs/testPlan_coverageReport.md)

### Business and User Experience

- [Tokenomics and Protocol Fee](docs/tokenomics_and_protocolFee.md)
- [UX Flow/User Journey](docs/userJourney.md)
- [Business Model and Monetization Plan](docs/businessModel_and_monetizationPlan.md)
- [Competitive Analysis](docs/competitiveAnalysis.md)
- [Visual Design Guide](docs/visualDesignGuide.md)

### Planning and Presentation

- [Roadmap](docs/roadmap.md)
- [FAQ and Pitch Deck](docs/FAQ_and_pitchDeck.md)
- [Hackathon Presentation Strategy](docs/hackathonPresentationStrategy.md)

---

## Contributing

We welcome contributions from the community!

1. **Fork the Repository**
2. **Create a Feature Branch:**
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. **Commit Your Changes:**
   ```bash
   git commit -m "Add feature: [your feature description]"
   ```
4. **Push to Your Branch:**
   ```bash
   git push origin feature/your-feature-name
   ```
5. **Submit a Pull Request**

Please ensure that your code is well-documented and passes all tests before submitting a PR.

---

## Roadmap

Our roadmap is outlined in the [Roadmap Document](docs/roadmap.md) and includes phases from pre-hackathon preparations to long-term ecosystem expansion.

---

## License

Fluxa is released under the MIT [License](LICENSE).
