# Detailed Technical Design Document

## 1. Introduction

Fluxa is a Hybrid Adaptive AMM with Personalized Yield Optimization designed for Solana’s high-speed, parallel execution environment. This document details the technical design and implementation strategy for each core module of the protocol, including concentrated liquidity math, dynamic liquidity curve adjustments, impermanent loss (IL) mitigation, yield optimization logic, order book integration, and fee distribution. The design leverages Solana’s account model and cross-program invocations (CPIs) to maintain modularity, scalability, and security.

## 2. System Overview

Fluxa consists of several interrelated modules deployed as Solana programs (using the Anchor framework). Each module is responsible for a distinct function:

- **AMM Core Module**: Manages liquidity pools, concentrated liquidity positions, fee accrual, and pricing curves.
- **Order Book Module**: Supports Serum-style limit order placement, order matching, and order management.
- **Impermanent Loss Mitigation Module**: Dynamically adjusts liquidity curves, triggers rebalancing, and monitors IL risk.
- **Personalized Yield Optimization Module**: Adjusts yield strategies based on user-selected risk profiles.
- **Insurance Fund Module**: Accumulates fees and provides capital to cover IL events.
- **External Integrations**: Interfaces with protocols like Jupiter, Marinade, Solend, and Kamino via CPIs.

The following sections detail the design and implementation for each module.

## 3. Module Designs & Algorithms

### 3.1 AMM Core Module

#### 3.1.1 Concentrated Liquidity & Pricing Curves

**Design Overview**:  
LPs can specify custom liquidity ranges. The module calculates the current pool price using a constant product formula adapted for concentrated liquidity. It supports fee accrual for each position.

**Algorithm Details**:

- **Liquidity Provision**:  
   For each liquidity position $L_i$ defined over a price range $[P_{min}, P_{max}]$:  
   Compute effective liquidity $L_{eff}$ based on the current price $P$.  
   Use the formula:

$$
L_{eff} =
\begin{cases}
L_i & \text{if } P \in [P_{min}, P_{max}] \\
0 & \text{otherwise}
\end{cases}
$$

- **Pricing Calculation**:  
   Utilize a modified constant product invariant $X \cdot Y = k$, where $X$ and $Y$ are adjusted based on liquidity ranges.

**Pseudocode Snippet**:

```rust
// Pseudocode for calculating effective liquidity
fn effective_liquidity(liquidity: u64, price: f64, p_min: f64, p_max: f64) -> u64 {
        if price >= p_min && price <= p_max {
                return liquidity;
        }
        return 0;
}
```

- **Fee Accrual**:  
   Fees are accumulated per swap and allocated to each liquidity position proportionally.  
   Track fee growth in a global state account and update individual positions accordingly.

#### 3.1.2 Solana Account Structure & CPI

- **Account Design**:

  - **Pool Account**: Holds state variables such as total liquidity, fee growth, and current price.
  - **Position Account**: Represents individual LP positions, including parameters $p_{min}, p_{max}$ and liquidity amount.

- **CPIs**:  
   Interact with external price oracles and other modules via CPI calls for real-time data.

### 3.2 Order Book Module

#### 3.2.1 Limit Order Functionality

**Design Overview**:  
Integrates Serum-style order books, allowing users to place limit orders directly on the pool.

**Core Functions**:

- **Order Placement**:  
   Validate and store limit orders with parameters (price, quantity, expiry).

- **Order Matching**:  
   Match incoming orders against existing orders. Implement a matching engine that processes orders sequentially.

- **Order Cancellation/Modification**:  
   Provide mechanisms to cancel or update orders.

**Pseudocode Snippet**:

```rust
// Pseudocode for placing an order
struct Order {
        id: u64,
        user: Pubkey,
        price: f64,
        quantity: u64,
        expiry: u64,
}

fn place_order(order: Order, order_book: &mut Vec<Order>) -> Result<()> {
        // Validate order parameters and add to order_book
        order_book.push(order);
        Ok(())
}
```

**Data Flow**:  
Orders are stored in a dedicated on-chain account (or a PDA) that represents the order book.  
Matching is triggered either by new orders or scheduled checks.

### 3.3 Impermanent Loss Mitigation Module

#### 3.3.1 Dynamic Liquidity Curve Adjustment

**Design Overview**:  
Continuously monitor market volatility and adjust liquidity ranges to minimize IL.

**Algorithm Details**:

- **Volatility Detection**:  
   Use external oracle feeds (aggregated prices) to assess market volatility.

- **Curve Adjustment**:  
   When volatility exceeds a threshold, recalibrate liquidity ranges for affected positions.  
   For example:

$$p_{min,new} = p_{min} \times (1-\delta), p_{max,new} = p_{max} \times (1+\delta)$$

where $\delta$ is a function of volatility.

**Pseudocode Snippet**:

```rust
// Pseudocode for dynamic curve adjustment
fn adjust_liquidity_range(current_min: f64, current_max: f64, volatility: f64) -> (f64, f64) {
        let delta = compute_delta(volatility);
        let new_min = current_min * (1.0 - delta);
        let new_max = current_max * (1.0 + delta);
        (new_min, new_max)
}

fn compute_delta(volatility: f64) -> f64 {
        // Example: Linear scaling with volatility
        return volatility * 0.05; // 5% adjustment factor per unit volatility
}
```

#### 3.3.2 Rebalancing and Insurance Fund Trigger

- **Rebalancing**:  
   Automatically trigger rebalancing events when liquidity falls outside optimal ranges.  
   Reallocate liquidity and update fee distributions.

- **Insurance Trigger**:  
   Monitor IL metrics in real time. If IL exceeds a predefined threshold, trigger an insurance fund payout.  
   Use stored fee reserves to compensate LPs.

### 3.4 Personalized Yield Optimization Module

#### 3.4.1 Risk Profile and Strategy Adjustment

**Design Overview**:  
Users select a risk profile (Conservative, Balanced, Aggressive). The module adjusts compounding frequency and liquidity rebalancing based on the chosen profile.

**Algorithm Details**:

- **Risk Profile Parameters**:  
   Each profile defines:

  - Target yield range
  - Compounding frequency
  - Rebalancing sensitivity

- **Strategy Switch**:  
   Use a decision matrix to adjust parameters:

$$Strategy=f(Risk Profile,Market Conditions)$$

**Pseudocode Snippet**:

```rust
enum RiskProfile {
        Conservative,
        Balanced,
        Aggressive,
}

fn adjust_yield_strategy(profile: RiskProfile, market_volatility: f64) -> (u64, f64) {
        match profile {
                RiskProfile::Conservative => (24, 0.02), // e.g., 24-hour compounding, 2% adjustment
                RiskProfile::Balanced => (12, 0.04),
                RiskProfile::Aggressive => (6, 0.08),
        }
}
```

#### 3.4.2 Performance Metrics and Analytics

- **Real-Time Metrics**:  
   Display current yield, historical performance, and risk exposure.

- **Data Aggregation**:  
   Collate data from AMM Core and external integrations to provide a comprehensive performance dashboard.

### 3.5 Insurance Fund Module

#### 3.5.1 Fee Accumulation & Payout Logic

**Design Overview**:  
A portion of trading fees is allocated to an insurance fund. This fund is used to compensate LPs in cases of significant impermanent loss.

**Algorithm Details**:

- **Fee Collection**:  
   A fixed percentage (e.g., 0.2–0.5%) of each trade is routed to the fund.

- **Payout Trigger**:  
   When IL exceeds a threshold, calculate payout based on the insured amount for each position.

**Pseudocode Snippet**:

```rust
fn collect_fee(trade_amount: u64, fee_rate: f64) -> u64 {
        return (trade_amount as f64 * fee_rate) as u64;
}

fn trigger_payout(insurance_balance: u64, il_excess: u64) -> u64 {
        // Example: Payout proportionally to IL excess, capped by available insurance funds
        return std::cmp::min(insurance_balance, il_excess);
}
```

### 3.6 External Integrations

#### 3.6.1 Integration via CPIs

- **Jupiter Aggregator**:  
   Route trades to the best available liquidity pools.

- **Marinade Finance**:  
   Leverage mSOL for auto-compounding yield strategies.

- **Solend/Kamino Finance**:  
   Extend yield options via lending/borrowing protocols.

**Implementation Considerations**:  
Use Solana CPIs to call external program functions.  
Implement error handling and fallback mechanisms for failed calls.

## 4. Fee Handling and Tokenomics

- **Fee Structure**:

  - **Trading Fees**: Collected from each swap and order execution.

- **Allocation**:

  - A portion goes to LPs as rewards.
  - A portion is diverted to the insurance fund.
  - A small cut may be reserved for protocol maintenance and governance rewards.

- **Tokenomics**:  
   Plan for a governance token that can be used to vote on fee adjustments, rebalancing strategies, and future protocol enhancements.

## 5. Testing & Validation

### 5.1 Unit Testing

- **Scope**:  
   Test individual functions (e.g., liquidity calculations, order matching).  
   Ensure correct behavior of math formulas and fee allocations.

- **Tools**:  
   Use Anchor’s testing framework, Rust unit tests.

### 5.2 Integration Testing

- **Scope**:  
   Simulate multi-module interactions (e.g., liquidity provision followed by order execution and IL rebalancing).

- **Tools**:  
   Deploy on Solana’s test validator; use automated scripts to simulate user actions.

### 5.3 Fuzz Testing & Property-Based Tests

- **Objective**:  
   Identify edge cases and potential vulnerabilities.  
   Ensure invariants (e.g., total liquidity consistency, fee integrity) are maintained.

## 6. Security Considerations

- **Invariant Checks**:  
   Ensure that at every state transition, key invariants (e.g., no LP withdraws more than deposited) are maintained.

- **Access Controls**:  
   Validate all CPI calls and ensure that only authorized accounts can perform sensitive operations.

- **Formal Verification**:  
   Consider formal methods for critical components (e.g., liquidity math, insurance payout logic).
