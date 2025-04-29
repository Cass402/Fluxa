# Fluxa Core Protocol Technical Design

**Document ID:** FLX-TECH-CORE-2025-001  
**Version:** 1.0  
**Date:** 2025-04-25  
**Status:** Draft  
**Classification:** Confidential

---

## Table of Contents

1. [Introduction](#1-introduction)

   1. [Purpose](#11-purpose)
   2. [Scope](#12-scope)
   3. [References](#13-references)
   4. [Terminology](#14-terminology)

2. [AMM Core Module Design](#2-amm-core-module-design)

   1. [Design Principles](#21-design-principles)
   2. [Constant Product Formula Extension](#22-constant-product-formula-extension)
   3. [Virtual Reserves and Ticks](#23-virtual-reserves-and-ticks)
   4. [Price Range Mathematics](#24-price-range-mathematics)

3. [Concentrated Liquidity Implementation](#3-concentrated-liquidity-implementation)

   1. [Tick System Design](#31-tick-system-design)
   2. [Position Representation](#32-position-representation)
   3. [Liquidity Aggregation](#33-liquidity-aggregation)
   4. [Range Order Mechanics](#34-range-order-mechanics)

4. [Pricing Curves and Swap Execution](#4-pricing-curves-and-swap-execution)

   1. [Price Calculation](#41-price-calculation)
   2. [Swap Execution Algorithm](#42-swap-execution-algorithm)
   3. [Multi-Hop Swaps](#43-multi-hop-swaps)
   4. [Price Impact Calculation](#44-price-impact-calculation)

5. [Solana Account Model Implementation](#5-solana-account-model-implementation)

   1. [Account Structure](#51-account-structure)
   2. [Program-Derived Addresses (PDAs)](#52-program-derived-addresses-pdas)
   3. [State Management](#53-state-management)
   4. [Account Size Optimization](#54-account-size-optimization)

6. [Fee Computation and Allocation](#6-fee-computation-and-allocation)

   1. [Fee Tier System](#61-fee-tier-system)
   2. [Fee Growth Tracking](#62-fee-growth-tracking)
   3. [Fee Distribution Algorithm](#63-fee-distribution-algorithm)
   4. [Protocol Fee Collection](#64-protocol-fee-collection)

7. [State Transitions and Transaction Flows](#7-state-transitions-and-transaction-flows)

   1. [Pool Initialization](#71-pool-initialization)
   2. [Position Management](#72-position-management)
   3. [Swap Transaction Flow](#73-swap-transaction-flow)
   4. [Failure Handling](#74-failure-handling)

8. [Position Management Algorithms](#8-position-management-algorithms)

   1. [Position Creation](#81-position-creation)
   2. [Position Modification](#82-position-modification)
   3. [Fee Collection](#83-fee-collection)
   4. [Position Closure](#84-position-closure)

9. [Mathematical Optimizations](#9-mathematical-optimizations)

   1. [Fixed-Point Arithmetic](#91-fixed-point-arithmetic)
   2. [Computational Optimizations](#92-computational-optimizations)
   3. [Storage Optimizations](#93-storage-optimizations)

10. [Security Invariants](#10-security-invariants)
    1. [Core Protocol Invariants](#101-core-protocol-invariants)
    2. [State Consistency Checks](#102-state-consistency-checks)

---

## 1. Introduction

### 1.1 Purpose

This document provides a comprehensive technical design for the Core Protocol of Fluxa, a next-generation decentralized finance (DeFi) protocol built on the Solana blockchain. It details the mathematical foundations, algorithms, and implementation strategies for the Automated Market Maker (AMM) core with concentrated liquidity.

### 1.2 Scope

This document covers:

- The fundamental mathematics of the AMM core module
- Concentrated liquidity implementation details
- Pricing curve algorithms and calculations
- Core data structures and Solana account models
- Fee computation and allocation mechanisms
- Core state transitions and transaction flows
- Liquidity position management algorithms

The following topics are addressed in separate technical documents:

- Impermanent Loss Mitigation (FLX-TECH-RISK-2025-001)
- Order Book Integration (FLX-TECH-FEATURES-2025-001)
- External Protocol Integration (FLX-TECH-INTEGRATION-2025-001)
- Security Analysis (FLX-SEC-2025-001)
- Testing Strategy (FLX-TEST-2025-001)

### 1.3 References

1. Fluxa Requirements Document (FLX-SRD-2025-001)
2. Fluxa Architecture Document (FLX-ARCH-2025-001)
3. Uniswap v3 Core Whitepaper
4. Solana Program Library Documentation
5. Anchor Framework Documentation

### 1.4 Terminology

| Term                   | Definition                                                                               |
| ---------------------- | ---------------------------------------------------------------------------------------- |
| AMM                    | Automated Market Maker - a protocol that enables asset trading using algorithmic pricing |
| Concentrated Liquidity | Liquidity provisioning model allowing LPs to specify price ranges for their liquidity    |
| Tick                   | Discrete price level in the protocol representing a specific price point                 |
| Impermanent Loss (IL)  | Temporary loss of asset value experienced by liquidity providers due to price divergence |
| Liquidity Position     | A user's allocation of assets to a specific price range in a liquidity pool              |
| Fee Growth             | Accumulation of trading fees over time, tracked per unit of liquidity                    |
| Swap                   | Exchange of one token for another through the AMM                                        |
| PDA                    | Program Derived Address - a deterministic account address derived from a Solana program  |

---

## 2. AMM Core Module Design

### 2.1 Design Principles

The Fluxa AMM Core is designed with the following principles:

1. **Capital Efficiency**: Optimize the usage of LP's capital through concentrated liquidity.
2. **Precision**: Ensure high mathematical precision in all calculations without compromising performance.
3. **Composability**: Design components that can be easily integrated with other modules.
4. **Scalability**: Optimize for Solana's parallel transaction processing capabilities.
5. **Security**: Maintain invariants at all points during execution.

### 2.2 Constant Product Formula Extension

The traditional constant product formula is extended to support concentrated liquidity:

In a standard AMM, the constant product formula is:

$$x \cdot y = k$$

Where:

- $x$ is the amount of token X
- $y$ is the amount of token Y
- $k$ is the constant product invariant

In Fluxa's concentrated liquidity model, we modify this to:

$$L = \sqrt{xy}$$

Where:

- $L$ is the liquidity value
- The relationship between tokens and price $(p = y/x)$ is:
  - $x = L / \sqrt{p}$
  - $y = L \cdot \sqrt{p}$

This allows us to track liquidity directly and compute token amounts based on price.

### 2.3 Virtual Reserves and Ticks

Fluxa employs a tick-based system similar to Uniswap v3 but optimized for Solana:

1. The price range is divided into discrete ticks representing price points.
2. Each tick represents a price according to the formula:
   $$P(i) = 1.0001^i$$
   where $i$ is the tick index.

3. Virtual reserves are calculated as the sum of all active liquidity at the current price:
   $$X_{virtual} = \sum_{p_{min} \leq p_{current} \leq p_{max}} \frac{L_i}{\sqrt{p_{current}}}$$
   $$Y_{virtual} = \sum_{p_{min} \leq p_{current} \leq p_{max}} L_i \cdot \sqrt{p_{current}}$$

### 2.4 Price Range Mathematics

For a position with liquidity $L$ over price range $[p_a, p_b]$ where $p_a < p_b$:

1. The amount of token X required for this position is:
   $$\Delta x = L \cdot \left( \frac{1}{\sqrt{p_a}} - \frac{1}{\sqrt{p_b}} \right)$$

2. The amount of token Y required is:
   $$\Delta y = L \cdot \left( \sqrt{p_b} - \sqrt{p_a} \right)$$

3. As the price moves within the range $[p_a, p_b]$, the token amounts held by the position change according to:
   $$x(p) = L \cdot \left( \frac{1}{\sqrt{p}} - \frac{1}{\sqrt{p_b}} \right)$$
   $$y(p) = L \cdot \left( \sqrt{p} - \sqrt{p_a} \right)$$

These formulas are crucial for calculating deposit amounts, withdrawal amounts, and swap results.

---

## 3. Concentrated Liquidity Implementation

### 3.1 Tick System Design

Fluxa implements a tick system with the following characteristics:

1. **Tick Spacing**: Configurable parameter per pool, defines the granularity of price ranges.
2. **Tick Initialization**: Ticks are initialized only when liquidity positions cross them.
3. **Tick Data Structure**:

```rust
pub struct Tick {
    // Liquidity net amount, can be positive or negative
    pub liquidity_net: i128,

    // Fee growth outside this tick
    pub fee_growth_outside_0: u128,
    pub fee_growth_outside_1: u128,

    // Last time this tick was crossed
    pub last_crossed_time: i64,

    // Whether this tick is initialized
    pub initialized: bool,
}
```

4. **Tick Crossing Logic**:
   When a price crosses a tick during a swap:
   - The tick's `fee_growth_outside` values are updated
   - The tick's `last_crossed_time` is updated
   - The active liquidity is updated by applying the tick's `liquidity_net` value

### 3.2 Position Representation

Positions are represented as follows:

```rust
pub struct Position {
    // Owner of the position
    pub owner: Pubkey,

    // Lower and upper tick indices defining the position price range
    pub tick_lower: i32,
    pub tick_upper: i32,

    // Liquidity amount in this position
    pub liquidity: u128,

    // Fee growth inside the position's range
    pub fee_growth_inside_0_last: u128,
    pub fee_growth_inside_1_last: u128,

    // Tokens owed to position from fees
    pub tokens_owed_0: u64,
    pub tokens_owed_1: u64,
}
```

Each position is stored in its own account, with the address derived as a PDA from:

- Pool ID
- Owner
- Lower tick
- Upper tick

This design allows efficient querying of positions by owner or pool.

### 3.3 Liquidity Aggregation

Active liquidity is aggregated across all positions that include the current price:

1. For each price $p_{current}$, active liquidity is:
   $$L_{active} = \sum_{position} L_{position} \cdot \mathbf{1}_{p_{min} \leq p_{current} \leq p_{max}}$$
   Where $\mathbf{1}$ is the indicator function that equals 1 when the condition is true, 0 otherwise.

2. This aggregation is efficiently implemented using the tick data structure:
   - When crossing a tick, add or subtract its `liquidity_net` value from the active liquidity
   - Track the current active liquidity in the pool state

### 3.4 Range Order Mechanics

Concentrated liquidity positions can function as range orders:

1. **Single-Sided Liquidity**: When a position is created entirely outside the current price:

   - If below current price: Only token Y is deposited
   - If above current price: Only token X is deposited

2. **Range Order Execution**:

   - As price moves into the position's range, the deposited token is gradually converted to the other token
   - When price moves through the entire range, complete conversion is achieved

3. **Implementation**:
   - Special handling in deposit logic for positions not spanning current price
   - Position assets are calculated using the formulas from section 2.4

---

## 4. Pricing Curves and Swap Execution

### 4.1 Price Calculation

The current price in a pool is stored as a sqrt price value for computational efficiency:

$$\sqrt{p} = \sqrt{\frac{y}{x}}$$

1. **Sqrt Price to Price Conversion**:
   $$p = (\sqrt{p})^2$$

2. **Price to Token Ratio**:
   $$\frac{y}{x} = p$$

3. **Tick to Price Conversion**:
   $$p(i) = 1.0001^i$$
   $$i(p) = \lfloor \log_{1.0001}(p) \rfloor$$

### 4.2 Swap Execution Algorithm

Swaps are executed using the following algorithm:

```rust
fn execute_swap(pool, amount_specified, sqrt_price_limit, zero_for_one) -> SwapResult {
    let mut state = initialize_swap_state(pool, amount_specified, sqrt_price_limit, zero_for_one);

    // Continue swapping until we've used the entire input or reached the price limit
    while state.amount_remaining > 0 && state.sqrt_price != sqrt_price_limit {
        // Find the next tick to cross
        let (next_tick, initialized) = get_next_initialized_tick(
            pool,
            state.tick_current,
            state.tick_spacing,
            zero_for_one
        );

        // Calculate the next sqrt price target
        let sqrt_price_target = compute_sqrt_price_target(
            next_tick,
            sqrt_price_limit,
            zero_for_one
        );

        // Compute how much can be swapped within this price range
        let (sqrt_price_next, amount_in, amount_out, fee_amount) = compute_swap_step(
            state.sqrt_price,
            sqrt_price_target,
            state.liquidity,
            state.amount_remaining,
            pool.fee
        );

        // Update state with the amount swapped
        state.sqrt_price = sqrt_price_next;
        state.amount_remaining -= amount_in;
        state.amount_calculated += amount_out;
        state.fee_growth += fee_amount;

        // If we've reached the next tick, cross it and update liquidity
        if sqrt_price_next == sqrt_price_target && initialized {
            update_tick_crossing(pool, next_tick, zero_for_one);
            state.tick_current = next_tick;
            state.liquidity = updated_liquidity_after_crossing(pool, next_tick, zero_for_one);
        }
    }

    // Update pool state
    update_pool_state(pool, state);

    return SwapResult {
        amount_in: amount_specified - state.amount_remaining,
        amount_out: state.amount_calculated,
        fee_amount: state.fee_growth
    };
}
```

The key mathematical function for computing a single step within a price range is:

```rust
fn compute_swap_step(
    sqrt_price_current: u128,
    sqrt_price_target: u128,
    liquidity: u128,
    amount_remaining: u128,
    fee_pct: u128
) -> (u128, u128, u128, u128) {
    // Compute the maximum amount that can be swapped to reach sqrt_price_target
    let max_amount = compute_max_amount_for_price_change(
        sqrt_price_current,
        sqrt_price_target,
        liquidity
    );

    // Determine if we'll use the entire remaining amount or just a portion
    let (use_entire_input, actual_amount) = if amount_remaining <= max_amount {
        (true, amount_remaining)
    } else {
        (false, max_amount)
    };

    // Apply fee
    let fee_amount = (actual_amount * fee_pct) / 1_000_000;
    let amount_after_fee = actual_amount - fee_amount;

    // Calculate new sqrt_price and amounts
    let sqrt_price_next = if use_entire_input {
        compute_sqrt_price_after_amount(
            sqrt_price_current,
            liquidity,
            amount_after_fee,
            zero_for_one
        )
    } else {
        sqrt_price_target
    };

    // Calculate exact input and output amounts
    let amount_in = calculate_amount_in(
        sqrt_price_current,
        sqrt_price_next,
        liquidity,
        zero_for_one
    );

    let amount_out = calculate_amount_out(
        sqrt_price_current,
        sqrt_price_next,
        liquidity,
        zero_for_one
    );

    return (sqrt_price_next, amount_in, amount_out, fee_amount);
}
```

The exact formulas for `calculate_amount_in` and `calculate_amount_out` depend on the direction of the swap:

For X to Y (zero_for_one = true):
$$\Delta x = \frac{L \cdot (\sqrt{p_b} - \sqrt{p_a})}{\sqrt{p_a} \cdot \sqrt{p_b}}$$
$$\Delta y = L \cdot (\sqrt{p_b} - \sqrt{p_a})$$

For Y to X (zero_for_one = false):
$$\Delta y = L \cdot (\sqrt{p_b} - \sqrt{p_a})$$
$$\Delta x = L \cdot \left( \frac{1}{\sqrt{p_a}} - \frac{1}{\sqrt{p_b}} \right)$$

### 4.3 Multi-Hop Swaps

Fluxa supports multi-hop swaps through integration with external routers:

1. **Router Integration**:

   - Expose a standard swap interface consumable by routers like Jupiter
   - Support atomic execution of multiple swaps in a single transaction

2. **Implementation**:
   - Define cross-program invocation (CPI) interfaces for router interaction
   - Support callback patterns to enable complex swap paths

### 4.4 Price Impact Calculation

Price impact is calculated as:

$$PriceImpact = \frac{P_{final} - P_{initial}}{P_{initial}} \times 100\%$$

Where:

- $P_{initial}$ is the price before the swap
- $P_{final}$ is the price after the swap

For user-facing applications, the protocol provides methods to calculate expected price impact before executing a swap.

---

## 5. Solana Account Model Implementation

### 5.1 Account Structure

The core protocol uses the following primary account types:

1. **Factory Account**:

   - Singleton account that manages pool creation
   - Stores protocol parameters
   - Controls protocol fee parameters

2. **Pool Account**:

   ```rust
   pub struct Pool {
       // Pool tokens
       pub token_0: Pubkey,
       pub token_1: Pubkey,

       // Current price and tick
       pub sqrt_price: u128,
       pub tick_current: i32,

       // Fee parameters
       pub fee: u32,  // Fee in hundredths of a bip (0.0001%)
       pub protocol_fee_0: u32,
       pub protocol_fee_1: u32,

       // Liquidity state
       pub liquidity: u128,

       // Fee growth trackers
       pub fee_growth_global_0: u128,
       pub fee_growth_global_1: u128,

       // Protocol fee accumulation
       pub protocol_fees_token_0: u64,
       pub protocol_fees_token_1: u64,

       // Tick spacing
       pub tick_spacing: u16,

       // Token vaults
       pub token_vault_0: Pubkey,
       pub token_vault_1: Pubkey,

       // Last observation
       pub observation_index: u16,
       pub observation_cardinality: u16,
       pub observation_cardinality_next: u16,
   }
   ```

3. **Position Account**:

   - Structure defined in section 3.2
   - One account per position
   - Created as PDAs for efficient querying

4. **Tick Account**:

   ```rust
   pub struct TickAccount {
       pub pool_id: Pubkey,
       pub tick_index: i32,
       pub tick: Tick  // Structure defined in section 3.1
   }
   ```

5. **Oracle Account**:

   ```rust
   pub struct Observation {
       pub block_timestamp: u32,
       pub sqrt_price: u128,
       pub tick_cumulative: i64,
       pub seconds_per_liquidity_cumulative: u128,
       pub initialized: bool,
   }

   pub struct OracleAccount {
       pub pool: Pubkey,
       pub observations: [Observation; 64],  // Configurable size
   }
   ```

### 5.2 Program-Derived Addresses (PDAs)

The protocol uses PDAs for deterministic account derivation:

1. **Pool PDA**:

   - Seeds: ["pool", token_0, token_1, fee_tier]
   - Ensures uniqueness per token pair and fee tier

2. **Position PDA**:

   - Seeds: ["position", pool_id, owner, tick_lower, tick_upper]
   - Enables efficient queries for positions by owner or pool

3. **Tick PDA**:

   - Seeds: ["tick", pool_id, tick_index]
   - Only initialized ticks have accounts created

4. **Token Vault PDAs**:
   - Seeds: ["vault", pool_id, token_address]
   - Pool-owned token accounts for holding liquidity

### 5.3 State Management

State transitions follow this pattern:

1. **Account Loading**:

   - Load and deserialize all relevant accounts at instruction start
   - Validate account relationships and permissions

2. **State Computation**:

   - Perform core calculations in memory
   - Maintain invariants throughout the computation

3. **Account Update**:

   - Update account data with new state values
   - Serialize and store

4. **Event Emission**:
   - Emit events for important state changes
   - Include relevant data for indexing and UI updates

### 5.4 Account Size Optimization

To optimize for Solana's account model:

1. **Fixed-Size Accounts**:

   - Pre-allocate accounts with fixed sizes
   - Avoid reallocation when possible

2. **Sparse Storage**:

   - Only initialize tick accounts when needed
   - Use space-efficient representations for large data structures

3. **Lookup Tables**:

   - Use Address Lookup Tables for common addresses
   - Reduce transaction size for complex operations

4. **Account Rent Exemption**:
   - Ensure all accounts maintain rent exemption
   - Calculate minimum balance requirements for each account type

---

## 6. Fee Computation and Allocation

### 6.1 Fee Tier System

Fluxa supports multiple fee tiers optimized for different token pair characteristics:

1. **Standard Fee Tiers**:

   - 0.01% - For stable pairs with minimal price volatility
   - 0.05% - For moderate volatility pairs
   - 0.30% - For standard pairs
   - 1.00% - For exotic pairs with high volatility

2. **Fee Tier Selection**:

   - Fee tier is selected at pool creation
   - Each token pair can have multiple pools with different fee tiers

3. **Implementation**:
   ```rust
   pub enum FeeTier {
       UltraLow = 100,     // 0.01%
       Low = 500,          // 0.05%
       Medium = 3000,      // 0.30%
       High = 10000,       // 1.00%
   }
   ```

### 6.2 Fee Growth Tracking

Fees are tracked per unit of liquidity to ensure fair distribution:

1. **Global Fee Growth**:

   - Track accumulated fees per unit of liquidity globally
   - Update on each swap:
     ```rust
     fn update_fee_growth_global(pool: &mut Pool, fee_amount_0: u64, fee_amount_1: u64) {
         if pool.liquidity > 0 {
             pool.fee_growth_global_0 += (fee_amount_0 as u128) << 64 / pool.liquidity;
             pool.fee_growth_global_1 += (fee_amount_1 as u128) << 64 / pool.liquidity;
         }
     }
     ```

2. **Tick Fee Growth Tracking**:

   - Track fees accumulated outside each initialized tick
   - Update when price crosses a tick

3. **Position Fee Calculation**:
   - Calculate fees owed to a position based on:
     - Current global fee growth
     - Fee growth inside the position's range
     - Position's liquidity amount

### 6.3 Fee Distribution Algorithm

Fees are distributed to LPs according to this algorithm:

```rust
fn calculate_fees_owed(
    position: &Position,
    pool: &Pool,
    tick_lower: &Tick,
    tick_upper: &Tick
) -> (u64, u64) {
    // Calculate fee growth inside the position's range
    let (fee_growth_inside_0, fee_growth_inside_1) = compute_fee_growth_inside(
        pool.tick_current,
        tick_lower.index,
        tick_upper.index,
        pool.fee_growth_global_0,
        pool.fee_growth_global_1,
        tick_lower.fee_growth_outside_0,
        tick_lower.fee_growth_outside_1,
        tick_upper.fee_growth_outside_0,
        tick_upper.fee_growth_outside_1
    );

    // Calculate fees earned since last collection
    let fee_delta_0 = position.liquidity *
        (fee_growth_inside_0 - position.fee_growth_inside_0_last);

    let fee_delta_1 = position.liquidity *
        (fee_growth_inside_1 - position.fee_growth_inside_1_last);

    // Convert to token units and add to previously uncollected fees
    let tokens_owed_0 = position.tokens_owed_0 + (fee_delta_0 >> 64) as u64;
    let tokens_owed_1 = position.tokens_owed_1 + (fee_delta_1 >> 64) as u64;

    return (tokens_owed_0, tokens_owed_1);
}
```

### 6.4 Protocol Fee Collection

A portion of fees can be allocated to the protocol:

1. **Protocol Fee Configuration**:

   - Set at the factory level as a fraction of the swap fee
   - Configurable per pool or globally

2. **Fee Collection Logic**:

   ```rust
   fn collect_protocol_fee(
       pool: &mut Pool,
       fee_amount_0: u64,
       fee_amount_1: u64
   ) -> (u64, u64) {
       let protocol_fee_0 = (fee_amount_0 * pool.protocol_fee_0 as u64) / 10_000;
       let protocol_fee_1 = (fee_amount_1 * pool.protocol_fee_1 as u64) / 10_000;

       pool.protocol_fees_token_0 += protocol_fee_0;
       pool.protocol_fees_token_1 += protocol_fee_1;

       return (protocol_fee_0, protocol_fee_1);
   }
   ```

3. **Protocol Fee Withdrawal**:
   - Only authorized addresses can withdraw protocol fees
   - Fees are sent to a designated treasury account

---

## 7. State Transitions and Transaction Flows

### 7.1 Pool Initialization

The pool initialization flow consists of:

1. **Pool Creation**:

   ```rust
   fn create_pool(
       factory: &mut Factory,
       token_0: Pubkey,
       token_1: Pubkey,
       fee_tier: FeeTier,
       sqrt_price_x96: u128
   ) -> Result<Pool> {
       // Ensure tokens are in canonical order
       let (token_0, token_1) = sort_tokens(token_0, token_1);

       // Create pool PDA
       let pool_seeds = [b"pool", token_0.as_ref(), token_1.as_ref(), fee_tier.as_ref()];
       let (pool_address, bump) = Pubkey::find_program_address(&pool_seeds, &program_id);

       // Create token vaults
       let vault_0 = create_token_vault(pool_address, token_0);
       let vault_1 = create_token_vault(pool_address, token_1);

       // Initialize tick spacing based on fee tier
       let tick_spacing = get_tick_spacing_for_fee_tier(fee_tier);

       // Calculate initial tick from sqrt price
       let initial_tick = price_to_tick(sqrt_price_x96_to_price(sqrt_price_x96));

       // Initialize pool state
       let pool = Pool {
           token_0,
           token_1,
           sqrt_price: sqrt_price_x96,
           tick_current: initial_tick,
           fee: fee_tier as u32,
           protocol_fee_0: factory.protocol_fee_0,
           protocol_fee_1: factory.protocol_fee_1,
           liquidity: 0,
           fee_growth_global_0: 0,
           fee_growth_global_1: 0,
           protocol_fees_token_0: 0,
           protocol_fees_token_1: 0,
           tick_spacing,
           token_vault_0: vault_0,
           token_vault_1: vault_1,
           observation_index: 0,
           observation_cardinality: 1,
           observation_cardinality_next: 1,
       };

       // Initialize the oracle with the first observation
       initialize_oracle(pool_address);

       return Ok(pool);
   }
   ```

2. **Oracle Initialization**:

   - Create an oracle account for the pool
   - Initialize the first observation with the current price

3. **Initial Liquidity Provision**:
   - First position must provide initial liquidity
   - Set price within the first position's range

### 7.2 Position Management

Position management involves these core flows:

1. **Position Creation**:

   ```rust
   fn create_position(
       pool: &mut Pool,
       owner: Pubkey,
       tick_lower: i32,
       tick_upper: i32,
       amount_0_desired: u64,
       amount_1_desired: u64,
       amount_0_min: u64,
       amount_1_min: u64
   ) -> Result<(Position, u64, u64)> {
       // Validate tick bounds
       validate_ticks(tick_lower, tick_upper, pool.tick_spacing);

       // Calculate liquidity amount from desired token amounts
       let liquidity = calculate_liquidity_from_amounts(
           pool.sqrt_price,
           tick_to_sqrt_price(tick_lower),
           tick_to_sqrt_price(tick_upper),
           amount_0_desired,
           amount_1_desired
       );

       // Calculate actual amounts needed for this liquidity
       let (amount_0, amount_1) = calculate_amounts_for_liquidity(
           pool.sqrt_price,
           tick_to_sqrt_price(tick_lower),
           tick_to_sqrt_price(tick_upper),
           liquidity
       );

       // Verify amounts against minimums
       if amount_0 < amount_0_min || amount_1 < amount_1_min {
           return Err(ErrorCode::SlippageExceeded);
       }

       // Create position account
       let position = Position {
           owner,
           tick_lower,
           tick_upper,
           liquidity,
           fee_growth_inside_0_last: 0,
           fee_growth_inside_1_last: 0,
           tokens_owed_0: 0,
           tokens_owed_1: 0,
       };

       // Update tick state
       update_tick(pool, tick_lower, liquidity as i128, true);
       update_tick(pool, tick_upper, -(liquidity as i128), true);

       // Update pool liquidity if position is in range
       if pool.tick_current >= tick_lower && pool.tick_current < tick_upper {
           pool.liquidity += liquidity;
       }

       // Transfer tokens to pool
       transfer_tokens_to_pool(owner, pool, amount_0, amount_1);

       return Ok((position, amount_0, amount_1));
   }
   ```

2. **Position Modification**:

   - Add or remove liquidity from an existing position
   - Recalculate token amounts based on current price
   - Update pool state and tick data

3. **Fee Collection**:
   - Calculate fees owed to a position
   - Transfer fees to the position owner
   - Update position's fee growth tracking

### 7.3 Swap Transaction Flow

The swap flow consists of:

1. **Pre-Swap Validation**:

   - Verify input parameters
   - Check slippage constraints

2. **Swap Execution**:

   - Execute the swap algorithm detailed in section 4.2
   - Calculate input amount, output amount, and fees

3. **State Updates**:

   - Update pool price, tick, and liquidity
   - Update fee growth accumulators
   - Update oracle observation

4. **Token Transfers**:
   - Transfer input token from user to pool
   - Transfer output token from pool to user

### 7.4 Failure Handling

The protocol implements these failure handling strategies:

1. **Atomic Transactions**:

   - All state updates within a single transaction are atomic
   - If any step fails, the entire transaction is reverted

2. **Constraint Checks**:

   - Input validation before state modification
   - Slippage protection for swaps and liquidity provision

3. **Error Codes**:
   - Specific error codes for different failure scenarios:
     ```rust
     pub enum ErrorCode {
         InvalidTickRange,
         InsufficientInputAmount,
         InsufficientLiquidity,
         SlippageExceeded,
         PriceLimitReached,
         NotInRange,
         ZeroLiquidity,
         InvalidFee,
         InvalidTokenOrder,
         // ...
     }
     ```

---

## 8. Position Management Algorithms

### 8.1 Position Creation

Creating a new position involves:

1. **Position Parameters**:

   - Token pair (implicitly through pool)
   - Price range defined by lower and upper ticks
   - Initial liquidity amount

2. **Deposit Calculation**:
   Given a liquidity amount $L$ and price range $[p_a, p_b]$:

   - If current price $p$ is below range $(p < p_a)$:
     - Only token Y is needed: $\Delta y = L \cdot (\sqrt{p_b} - \sqrt{p_a})$
   - If current price $p$ is above range $(p > p_b)$:
     - Only token X is needed: $\Delta x = L \cdot \left( \frac{1}{\sqrt{p_a}} - \frac{1}{\sqrt{p_b}} \right)$
   - If current price $p$ is within range $(p_a \leq p \leq p_b)$:
     - Both tokens are needed:
       $\Delta x = L \cdot \left( \frac{1}{\sqrt{p}} - \frac{1}{\sqrt{p_b}} \right)$
       $\Delta y = L \cdot (\sqrt{p} - \sqrt{p_a})$

3. **Tick Updates**:
   - Increase net liquidity at lower tick
   - Decrease net liquidity at upper tick
   - Initialize ticks if needed

### 8.2 Position Modification

Modifying a position involves:

1. **Liquidity Addition**:

   - Calculate additional token amounts required
   - Add liquidity to position
   - Update tick state and pool liquidity

2. **Liquidity Removal**:

   - Calculate token amounts to return
   - Subtract liquidity from position
   - Update tick state and pool liquidity
   - Collect any accrued fees

3. **Implementation**:

   ```rust
   fn modify_position(
       pool: &mut Pool,
       position: &mut Position,
       liquidity_delta: i128,
       collect_fees: bool
   ) -> Result<(u64, u64, u64, u64)> {
       // Collect fees if requested
       let (fees_token_0, fees_token_1) = if collect_fees {
           collect_position_fees(pool, position)
       } else {
           (0, 0)
       };

       // Calculate token amounts for liquidity change
       let (amount_0_delta, amount_1_delta) = if liquidity_delta != 0 {
           let (amount_0, amount_1) = calculate_amounts_for_liquidity_change(
               pool.sqrt_price,
               tick_to_sqrt_price(position.tick_lower),
               tick_to_sqrt_price(position.tick_upper),
               liquidity_delta
           );

           // Update tick state
           update_tick(pool, position.tick_lower, liquidity_delta, false);
           update_tick(pool, position.tick_upper, -liquidity_delta, false);

           // Update pool liquidity if position is in range
           if pool.tick_current >= position.tick_lower && pool.tick_current < position.tick_upper {
               if liquidity_delta > 0 {
                   pool.liquidity += liquidity_delta as u128;
               } else {
                   pool.liquidity -= (-liquidity_delta) as u128;
               }
           }

           // Update position liquidity
           if liquidity_delta > 0 {
               position.liquidity += liquidity_delta as u128;
           } else {
               position.liquidity -= (-liquidity_delta) as u128;
           }

           (amount_0, amount_1)
       } else {
           (0, 0)
       };

       return Ok((amount_0_delta, amount_1_delta, fees_token_0, fees_token_1));
   }
   ```

### 8.3 Fee Collection

Fee collection is implemented as:

```rust
fn collect_position_fees(
    pool: &Pool,
    position: &mut Position
) -> Result<(u64, u64)> {
    // Get ticks
    let tick_lower = get_tick(pool.address, position.tick_lower)?;
    let tick_upper = get_tick(pool.address, position.tick_upper)?;

    // Calculate fee growth inside the position's range
    let (fee_growth_inside_0, fee_growth_inside_1) = compute_fee_growth_inside(
        pool.tick_current,
        position.tick_lower,
        position.tick_upper,
        pool.fee_growth_global_0,
        pool.fee_growth_global_1,
        tick_lower.fee_growth_outside_0,
        tick_lower.fee_growth_outside_1,
        tick_upper.fee_growth_outside_0,
        tick_upper.fee_growth_outside_1
    );

    // Calculate fees earned
    let fee_delta_0 = calculate_fee_delta(
        position.liquidity,
        fee_growth_inside_0,
        position.fee_growth_inside_0_last
    );

    let fee_delta_1 = calculate_fee_delta(
        position.liquidity,
        fee_growth_inside_1,
        position.fee_growth_inside_1_last
    );

    // Update position's fee tracking
    position.fee_growth_inside_0_last = fee_growth_inside_0;
    position.fee_growth_inside_1_last = fee_growth_inside_1;

    // Add to tokens owed
    position.tokens_owed_0 += fee_delta_0;
    position.tokens_owed_1 += fee_delta_1;

    // Return the collected amounts
    let amount_0 = position.tokens_owed_0;
    let amount_1 = position.tokens_owed_1;

    // Reset tokens owed
    position.tokens_owed_0 = 0;
    position.tokens_owed_1 = 0;

    return Ok((amount_0, amount_1));
}
```

### 8.4 Position Closure

Closing a position involves:

1. **Liquidity Removal**:

   - Remove all liquidity from the position
   - Calculate token amounts to return

2. **Fee Collection**:

   - Collect any remaining fees

3. **Position Cleanup**:

   - Close the position account
   - Reclaim rent if applicable

4. **Implementation**:

   ```rust
   fn close_position(
       pool: &mut Pool,
       position: &Position
   ) -> Result<(u64, u64, u64, u64)> {
       // Remove all liquidity
       let (amount_0, amount_1, fees_0, fees_1) = modify_position(
           pool,
           position,
           -(position.liquidity as i128),
           true
       )?;

       // Close the position account
       close_account(position.address, position.owner);

       return Ok((amount_0, amount_1, fees_0, fees_1));
   }
   ```

---

## 9. Mathematical Optimizations

### 9.1 Fixed-Point Arithmetic

Fluxa uses fixed-point arithmetic for precision:

1. **Q64.96 Format**:

   - For sqrt prices: 64 integer bits, 96 fractional bits
   - Allows representation of a wide price range with high precision

2. **Q128.128 Format**:

   - For fee growth tracking
   - Provides sufficient precision for fee accumulation over time

3. **Implementation**:

   ```rust
   // Convert from price to sqrt_price_x96
   fn price_to_sqrt_price_x96(price: f64) -> u128 {
       let sqrt_price = (price as f64).sqrt();
       return (sqrt_price * (1u128 << 96) as f64) as u128;
   }

   // Convert from sqrt_price_x96 to price
   fn sqrt_price_x96_to_price(sqrt_price_x96: u128) -> f64 {
       let sqrt_price = (sqrt_price_x96 as f64) / (1u128 << 96) as f64;
       return sqrt_price * sqrt_price;
   }
   ```

### 9.2 Computational Optimizations

Key optimizations include:

1. **Sqrt Price Representation**:

   - Store and manipulate sqrt(price) instead of price
   - Simplifies many liquidity calculations

2. **Tick Bitmap**:

   - Use bitmap to efficiently find initialized ticks
   - Optimize tick traversal during swaps

3. **Logarithm Approximations**:
   - Use efficient algorithms for log base 1.0001 calculations
   - Implement lookup tables for common conversions

### 9.3 Storage Optimizations

To optimize on-chain storage:

1. **Sparse Tick Storage**:

   - Only create accounts for initialized ticks
   - Use binary search through tick bitmap

2. **Oracle Observation Compression**:

   - Use time deltas instead of absolute timestamps
   - Pack multiple observations into single accounts

3. **Account Reuse**:
   - Reuse account structures where possible
   - Implement account recycling for common operations

---

## 10. Security Invariants

### 10.1 Core Protocol Invariants

The protocol maintains these invariants at all times:

1. **Liquidity Consistency**:

   - Sum of all liquidity deltas across ticks equals zero
   - Pool liquidity equals sum of in-range position liquidity

2. **Token Conservation**:

   - Total tokens in the pool equals sum of tokens in all positions
   - Swaps conserve value (minus fees)

3. **Price Monotonicity**:

   - Price only moves monotonically within a single swap
   - Tick traversal follows sequential order

4. **Fee Correctness**:
   - All fees are accounted for and correctly distributed
   - Fee growth tracking is precise and fair

### 10.2 State Consistency Checks

The implementation enforces these consistency checks:

1. **Range Validation**:

   - Lower tick < Upper tick
   - Ticks are properly aligned to tick spacing

2. **Value Checks**:

   - Liquidity is always non-negative
   - Prices are always positive

3. **Authorization Checks**:

   - Only position owners can modify or collect fees
   - Only authorized addresses can withdraw protocol fees

4. **Arithmetic Checks**:
   - Overflow and underflow protection on all calculations
   - Proper handling of edge cases in fixed-point math

## 11. Performance Benchmarks

### 11.1 Computational Complexity

The computational complexity of key operations is:

| Operation        | Time Complexity | Space Complexity | Notes                                                              |
| ---------------- | --------------- | ---------------- | ------------------------------------------------------------------ |
| Swap             | O(log n)        | O(1)             | Where n is the number of initialized ticks that need to be crossed |
| Add Liquidity    | O(1)            | O(1)             | Constant time operation with fixed account access                  |
| Remove Liquidity | O(1)            | O(1)             | Constant time operation with fixed account access                  |
| Collect Fees     | O(1)            | O(1)             | Requires reading two ticks and the position                        |
| Create Pool      | O(1)            | O(1)             | Fixed cost regardless of pool parameters                           |
| Find Tick        | O(log m)        | O(1)             | Where m is the number of ticks in the searchable range             |

### 11.2 Gas Optimization Targets

The protocol aims to meet the following performance targets on Solana:

| Operation              | Target Compute Units | Target Lamports (@ 1 CU = 0.00001 SOL) |
| ---------------------- | -------------------- | -------------------------------------- |
| Swap (No Cross)        | < 100,000 CU         | < 0.001 SOL                            |
| Swap (With Tick Cross) | < 200,000 CU         | < 0.002 SOL                            |
| Add Liquidity          | < 150,000 CU         | < 0.0015 SOL                           |
| Remove Liquidity       | < 150,000 CU         | < 0.0015 SOL                           |
| Collect Fees           | < 80,000 CU          | < 0.0008 SOL                           |
| Create Pool            | < 300,000 CU         | < 0.003 SOL                            |

### 11.3 Throughput Estimates

Based on Solana's capacity and the protocol design:

- **Maximum Swaps per Second**: ~1,500 (based on average of 150,000 CU per swap)
- **Maximum Positions Created per Second**: ~1,000
- **Maximum Pool State Updates per Second**: ~2,000

## 12. Implementation Milestones

### 12.1 Core Protocol Development

1. **Foundation Layer** (Week 1-2)

   - Basic account structures
   - Token vault management
   - Pool initialization

2. **Liquidity Management** (Week 3-4)

   - Tick system implementation
   - Position creation and modification
   - Liquidity tracking

3. **Swap Engine** (Week 5-6)

   - Pricing algorithm implementation
   - Swap execution logic
   - Tick crossing handling

4. **Fee System** (Week 7-8)

   - Fee growth tracking
   - Fee collection mechanisms
   - Protocol fee handling

5. **Oracle and Analytics** (Week 9-10)
   - Oracle observations
   - TWAP calculations
   - Liquidity depth metrics

### 12.2 Testing Strategy

1. **Unit Test Coverage**:

   - Aim for >90% code coverage
   - Focus on mathematical edge cases
   - Tick crossing exhaustive testing

2. **Integration Testing**:

   - End-to-end workflow validation
   - Multi-transaction scenarios
   - Simulated user interactions

3. **Stress Testing**:
   - High volume swap simulation
   - Concurrent position management
   - Edge case detection

### 12.3 Deployment Phases

1. **Phase 1: Devnet Deployment**

   - Deploy full protocol to Solana devnet
   - Conduct public testing and gather feedback
   - Optimize based on real-world usage patterns

2. **Phase 2: Testnet Refinement**

   - Deploy to Solana testnet with optimizations
   - Extended testing period with incentivized participants
   - Final performance tuning

3. **Phase 3: Mainnet Launch**
   - Phased rollout starting with limited pools
   - Gradual expansion to additional token pairs
   - Full activation of all protocol features

## 13. Future Enhancements

### 13.1 Core Protocol Enhancements

1. **Dynamic Fee Tiers**

   - Automatically adjust fee tiers based on volatility
   - Implement governance mechanisms for fee adjustments

2. **Advanced Oracle Functions**

   - Support for geometric mean TWAP
   - Volatility calculation built into oracle
   - Historical price range tracking

3. **Optimistic Swaps**
   - Execute swaps with optimistic updates
   - Verify correctness afterward to reduce latency

### 13.2 Technical Optimizations

1. **Tick Bitmap Improvements**

   - Enhanced tick bitmap for faster tick traversal
   - Cached tick data for commonly accessed price points

2. **Transaction Batching**

   - Support for executing multiple swaps in a single transaction
   - Batch updates for positions with shared parameters

3. **Storage Optimization**
   - Further compression of position and tick data
   - Intelligent data pruning for inactive positions

### 13.3 Integration Enhancements

1. **Native Flash Loan Capability**

   - Enable flash loans directly from pool liquidity
   - Implement security measures to prevent exploitation

2. **Cross-Pool Position Management**

   - Unified position management across multiple pools
   - Portfolio-based liquidity optimization

3. **Custom Curve Extensions**
   - Support for non-constant product curves
   - Hybrid curve models for specialized pairs

## 14. Implementation Alternatives Considered

### 14.1 Alternative Mathematical Models

1. **Discrete Liquidity Ticks vs. Continuous Functions**

   - Considered using continuous liquidity functions
   - Chose discrete ticks for computational efficiency and similarity to established models
   - Trade-off: Less granular price discovery but more efficient computation

2. **Fixed vs. Variable Tick Spacing**
   - Considered variable tick spacing based on price level
   - Selected fixed tick spacing per pool for simplicity and gas efficiency
   - Trade-off: Less flexibility but more predictable gas costs

### 14.2 State Management Alternatives

1. **Global vs. Per-Position Fee Tracking**

   - Considered directly tracking fees per position
   - Chose global fee tracking with position-relative calculations
   - Trade-off: More complex fee calculation but vastly reduced storage requirements

2. **Account Structure Designs**
   - Considered monolithic pool accounts containing all data
   - Selected modular design with separate position and tick accounts
   - Trade-off: More account lookups but better parallel execution and upgradability

### 14.3 Swap Execution Alternatives

1. **Linear Swap Path vs. Binary Search**

   - Considered binary search approach for tick crossing
   - Selected linear traversal for simplicity and guaranteed correctness
   - Trade-off: Potentially higher computation but predictable execution

2. **Exact vs. Approximated Math**
   - Considered approximation algorithms for some calculations
   - Chose exact calculation with fixed-point arithmetic
   - Trade-off: Higher computational cost but guaranteed precision

## 15. Appendices

### 15.1 Mathematical Derivations

#### 15.1.1 Sqrt Price and Liquidity Relationship

Starting from the constant product formula:

$$x \cdot y = k$$

We define liquidity $L$ as:

$$L = \sqrt{k}$$

For a given price $p = y/x$, we can derive:

$$x \cdot y = L^2$$
$$x \cdot p \cdot x = L^2$$
$$x^2 \cdot p = L^2$$
$$x = \frac{L}{\sqrt{p}}$$

Similarly:
$$y = L \cdot \sqrt{p}$$

#### 15.1.2 Token Amounts for a Price Range

For a position with liquidity $L$ in range $[p_a, p_b]$, the amount of tokens needed is derived from the integral of the marginal token amounts across the range:

$$\Delta x = L \cdot \int_{p_a}^{p_b} \frac{1}{2\sqrt{p^3}} dp = L \cdot \left[ -\frac{1}{\sqrt{p}} \right]_{p_a}^{p_b} = L \cdot \left( \frac{1}{\sqrt{p_a}} - \frac{1}{\sqrt{p_b}} \right)$$

$$\Delta y = L \cdot \int_{p_a}^{p_b} \frac{1}{2\sqrt{p}} dp = L \cdot \left[ \sqrt{p} \right]_{p_a}^{p_b} = L \cdot \left( \sqrt{p_b} - \sqrt{p_a} \right)$$

### 15.2 Fixed-Point Arithmetic Reference

#### 15.2.1 Q64.96 Operations

The protocol uses Q64.96 fixed-point representation for sqrt prices, where:

- 64 bits for the integer part
- 96 bits for the fractional part

Basic operations:

```rust
// Q64.96 addition
fn add_q64_96(a: u128, b: u128) -> u128 {
    a.checked_add(b).expect("Q64.96 addition overflow")
}

// Q64.96 subtraction
fn sub_q64_96(a: u128, b: u128) -> u128 {
    a.checked_sub(b).expect("Q64.96 subtraction underflow")
}

// Q64.96 multiplication (returns Q64.96)
fn mul_q64_96(a: u128, b: u128) -> u128 {
    let result = (a as u256) * (b as u256);
    ((result >> 96) & ((1 << 160) - 1)) as u128
}

// Q64.96 division (returns Q64.96)
fn div_q64_96(a: u128, b: u128) -> u128 {
    ((a as u256) << 96) / (b as u256) as u128
}
```

#### 15.2.2 Conversion Functions

```rust
// Convert from price (floating point) to Q64.96 sqrt price
fn price_to_sqrt_price_q64_96(price: f64) -> u128 {
    let sqrt_price = price.sqrt();
    (sqrt_price * (1u128 << 96) as f64) as u128
}

// Convert from Q64.96 sqrt price to price (floating point)
fn sqrt_price_q64_96_to_price(sqrt_price: u128) -> f64 {
    let sqrt_price_float = (sqrt_price as f64) / (1u128 << 96) as f64;
    sqrt_price_float * sqrt_price_float
}

// Convert from tick index to sqrt price
fn tick_to_sqrt_price_q64_96(tick: i32) -> u128 {
    let value = 1.0001f64.powi(tick);
    price_to_sqrt_price_q64_96(value)
}
```

### 15.3 Error Codes Reference

| Error Code              | Value  | Description                                 |
| ----------------------- | ------ | ------------------------------------------- |
| InvalidTickRange        | 0x1001 | Tick lower must be less than tick upper     |
| InvalidTickSpacing      | 0x1002 | Ticks must be multiples of tick spacing     |
| InsufficientLiquidity   | 0x1003 | Insufficient liquidity for operation        |
| InsufficientInputAmount | 0x1004 | Input amount is insufficient                |
| InvalidSqrtPriceLimit   | 0x1005 | Provided sqrt price limit is invalid        |
| PriceLimitReached       | 0x1006 | Price limit reached during swap             |
| NotOwner                | 0x1007 | Caller is not the position owner            |
| ZeroLiquidity           | 0x1008 | Cannot create position with zero liquidity  |
| PositionNotFound        | 0x1009 | The specified position does not exist       |
| PoolNotFound            | 0x100A | The specified pool does not exist           |
| InvalidTokenOrder       | 0x100B | Tokens must be provided in correct order    |
| ArithmeticError         | 0x100C | Arithmetic calculation error                |
| SlippageExceeded        | 0x100D | The operation exceeded allowable slippage   |
| DeadlineExceeded        | 0x100E | Transaction deadline has passed             |
| InvalidFee              | 0x100F | The fee value is not supported              |
| PoolAlreadyExists       | 0x1010 | A pool with these parameters already exists |

### 15.4 Solana Program Interface

#### 15.4.1 Core Instructions

```rust
pub enum FluxaInstruction {
    /// Initialize the protocol factory
    ///
    /// Accounts:
    /// 0. `[writable]` Factory account
    /// 1. `[signer]` Payer account (for rent)
    /// 2. `[]` System program
    InitializeFactory {
        protocol_fee_0: u32,
        protocol_fee_1: u32,
    },

    /// Create a new pool
    ///
    /// Accounts:
    /// 0. `[]` Factory account
    /// 1. `[writable]` Pool account (PDA)
    /// 2. `[]` Token 0 mint
    /// 3. `[]` Token 1 mint
    /// 4. `[writable]` Token 0 vault (PDA)
    /// 5. `[writable]` Token 1 vault (PDA)
    /// 6. `[writable]` Oracle account (PDA)
    /// 7. `[signer]` Payer account (for rent)
    /// 8. `[]` Token program
    /// 9. `[]` System program
    CreatePool {
        fee: u32,
        tick_spacing: u16,
        initial_sqrt_price: u128,
    },

    /// Create a liquidity position
    ///
    /// Accounts:
    /// 0. `[writable]` Pool account
    /// 1. `[writable]` Position account (PDA to be created)
    /// 2. `[writable]` Tick lower account (PDA, may need creation)
    /// 3. `[writable]` Tick upper account (PDA, may need creation)
    /// 4. `[writable]` Token 0 vault
    /// 5. `[writable]` Token 1 vault
    /// 6. `[writable]` Owner token 0 account
    /// 7. `[writable]` Owner token 1 account
    /// 8. `[signer]` Owner account
    /// 9. `[]` Token program
    /// 10. `[]` System program
    CreatePosition {
        tick_lower: i32,
        tick_upper: i32,
        amount_0_desired: u64,
        amount_1_desired: u64,
        amount_0_min: u64,
        amount_1_min: u64,
        deadline: i64,
    },

    /// Add liquidity to an existing position
    ///
    /// Accounts:
    /// 0. `[writable]` Pool account
    /// 1. `[writable]` Position account
    /// 2. `[writable]` Tick lower account
    /// 3. `[writable]` Tick upper account
    /// 4. `[writable]` Token 0 vault
    /// 5. `[writable]` Token 1 vault
    /// 6. `[writable]` Owner token 0 account
    /// 7. `[writable]` Owner token 1 account
    /// 8. `[signer]` Owner account
    /// 9. `[]` Token program
    AddLiquidity {
        amount_0_desired: u64,
        amount_1_desired: u64,
        amount_0_min: u64,
        amount_1_min: u64,
        deadline: i64,
    },

    /// Remove liquidity from a position
    ///
    /// Accounts:
    /// 0. `[writable]` Pool account
    /// 1. `[writable]` Position account
    /// 2. `[writable]` Tick lower account
    /// 3. `[writable]` Tick upper account
    /// 4. `[writable]` Token 0 vault
    /// 5. `[writable]` Token 1 vault
    /// 6. `[writable]` Owner token 0 account
    /// 7. `[writable]` Owner token 1 account
    /// 8. `[signer]` Owner account
    /// 9. `[]` Token program
    RemoveLiquidity {
        liquidity: u128,
        amount_0_min: u64,
        amount_1_min: u64,
        deadline: i64,
    },

    /// Collect fees from a position
    ///
    /// Accounts:
    /// 0. `[]` Pool account
    /// 1. `[writable]` Position account
    /// 2. `[]` Tick lower account
    /// 3. `[]` Tick upper account
    /// 4. `[writable]` Token 0 vault
    /// 5. `[writable]` Token 1 vault
    /// 6. `[writable]` Owner token 0 account
    /// 7. `[writable]` Owner token 1 account
    /// 8. `[signer]` Owner account
    /// 9. `[]` Token program
    CollectFees {
        amount_0_max: u64,
        amount_1_max: u64,
    },

    /// Execute a swap
    ///
    /// Accounts:
    /// 0. `[writable]` Pool account
    /// 1. `[writable]` Oracle account
    /// 2. `[writable]` Token in vault
    /// 3. `[writable]` Token out vault
    /// 4. `[writable]` User token in account
    /// 5. `[writable]` User token out account
    /// 6. `[signer]` User account
    /// 7. `[]` Token program
    Swap {
        amount_specified: u64,
        sqrt_price_limit: u128,
        zero_for_one: bool,
        exact_input: bool,
        amount_minimum: u64,
        deadline: i64,
    },

    /// Collect protocol fees
    ///
    /// Accounts:
    /// 0. `[]` Factory account
    /// 1. `[writable]` Pool account
    /// 2. `[writable]` Token 0 vault
    /// 3. `[writable]` Token 1 vault
    /// 4. `[writable]` Fee recipient token 0 account
    /// 5. `[writable]` Fee recipient token 1 account
    /// 6. `[signer]` Fee authority account
    /// 7. `[]` Token program
    CollectProtocolFees {
        amount_0: u64,
        amount_1: u64,
    }
}
```

#### 15.4.2 Event Definitions

```rust
// Core pool events
#[event]
pub struct PoolCreatedEvent {
    pub pool: Pubkey,
    pub token_0: Pubkey,
    pub token_1: Pubkey,
    pub fee: u32,
    pub tick_spacing: u16,
    pub initial_sqrt_price: u128,
}

// Position events
#[event]
pub struct PositionCreatedEvent {
    pub position: Pubkey,
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: u128,
    pub amount_0: u64,
    pub amount_1: u64,
}

#[event]
pub struct LiquidityChangedEvent {
    pub position: Pubkey,
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub liquidity_delta: i128,
    pub amount_0: u64,
    pub amount_1: u64,
}

#[event]
pub struct FeesCollectedEvent {
    pub position: Pubkey,
    pub owner: Pubkey,
    pub amount_0: u64,
    pub amount_1: u64,
}

// Swap events
#[event]
pub struct SwapEvent {
    pub pool: Pubkey,
    pub sender: Pubkey,
    pub token_in: Pubkey,
    pub token_out: Pubkey,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub sqrt_price_before: u128,
    pub sqrt_price_after: u128,
    pub liquidity_before: u128,
    pub liquidity_after: u128,
    pub tick_before: i32,
    pub tick_after: i32,
}

// Protocol events
#[event]
pub struct ProtocolFeesCollectedEvent {
    pub pool: Pubkey,
    pub recipient: Pubkey,
    pub amount_0: u64,
    pub amount_1: u64,
}
```

---
