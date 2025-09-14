# Fluxa AI Coding Agent Instructions

## Project Overview

Fluxa is a hybrid adaptive AMM (Automated Market Maker) and personalized yield optimizer on Solana, built using Anchor framework. It combines concentrated liquidity, integrated order books, and dynamic impermanent loss mitigation with enterprise-grade security features.

## Architecture & Core Components

### Program Structure

- **fluxa_core**: Main Anchor program (`programs/fluxa_core/src/`)
  - `math/`: Q64.64 fixed-point arithmetic engine with deterministic precision
  - `state/`: Account structures (factory, pool, position, tick)
  - `security/`: PDA authorities, multisig, timelock systems
  - `utils/constants.rs`: Protocol bounds and configuration

### Key Architectural Patterns

**Fixed-Point Mathematics**: All financial calculations use Q64.64 (128-bit with 64 fractional bits) instead of floating-point for deterministic cross-platform behavior. See `core_arithmetic.rs` for the mathematical foundation.

**Concentrated Liquidity Model**: Following Uniswap V3 patterns with tick-based pricing:

- Ticks represent discrete price levels (0.01% increments)
- Binary exponentiation for tick-to-price conversion using precomputed coefficients
- Square root price representation for numerical stability

**Modular State Management**:

- Factory manages protocol-wide configuration and sharding
- Pool handles per-pair trading logic and volatility tracking
- Position manages LP stakes with Merkle batching for gas efficiency
- Tick manages granular price level data with compression

## Development Workflow

### Docker-Based Development

Always use the containerized environment:

```bash
make up          # Start development environment
make dev-shell   # Enter container shell
make test        # Run all tests
make deploy      # Deploy to localnet
```

### Build System

- **Anchor**: Primary build tool (`anchor build`, `anchor test`)
- **Cargo**: Rust compilation with custom toolchain (see `rust-toolchain.toml`)
- **Docker**: Ensures consistent Solana CLI (v2.1.0) and Anchor (v0.31.0) versions

### Test Structure

- Unit tests: Alongside source files in each module
- Integration tests: `tests/` directory with full program interactions
- Property-based testing: Uses `proptest` for mathematical invariants
- Fuzz testing: Randomized inputs for edge case discovery

## Critical Code Patterns

### Error Handling

Use granular error enums from `error.rs` - never panic or use unwrap():

```rust
// Good: Checked arithmetic with specific errors
amount.checked_add(fee).ok_or(MathError::Overflow)?

// Bad: Panicking arithmetic
amount + fee  // Could panic on overflow
```

### Fixed-Point Arithmetic

Always use Q64x64 wrapper types for financial calculations:

```rust
// Good: Deterministic fixed-point math
let price = Q64x64::from_int(100);
let result = price.checked_mul(rate)?;

// Bad: Floating-point (non-deterministic across nodes)
let price = 100.0 * rate;
```

### Account Management

Follow zero-copy patterns for large accounts:

```rust
#[account(zero_copy)]
pub struct PoolAccount {
    // Large structs use zero_copy for performance
}

#[derive(Pod, Zeroable)] // Required for zero_copy
pub struct TickData {
    // Tick data with bytemuck compatibility
}
```

## Security Considerations

### Mathematical Safety

- All arithmetic operations are checked for overflow/underflow
- Price bounds enforced via `MIN_SQRT_X64`/`MAX_SQRT_X64` constants
- Liquidity calculations use mul_div to prevent intermediate overflow

### Authority Management

- Multi-layered PDA security with timelock delays
- Emergency pause mechanisms with automatic timeout
- Multisig requirements for critical operations
- Audit trails for all authority changes

### MEV Protection

- Rate limiting on tick crossings and suspicious activity detection
- Volume spike anomaly detection
- Flash loan pattern recognition
- Bitwise status flags for efficient state management

## Common Tasks

### Adding New Mathematical Operations

1. Implement in `core_arithmetic.rs` using checked arithmetic
2. Add comprehensive unit tests with edge cases
3. Document the mathematical rationale and precision requirements
4. Ensure operations maintain Q64.64 invariants

### Extending State Structures

1. Update relevant account struct in `state/` modules
2. Maintain zero_copy compatibility with Pod/Zeroable
3. Update account size calculations in `constants.rs`
4. Consider sharding implications for large accounts

### Adding New Error Types

1. Add to appropriate error enum in `error.rs`
2. Include descriptive message for debugging
3. Use in checked operations instead of panicking
4. Document the error conditions in function docs

## Integration Points

### External Dependencies

- **Anchor**: Framework and macro system (v0.31.0)
- **SPL Token**: Solana token standard integration
- **Solana Program**: Core Solana primitives and syscalls
- **ethnum**: U256 for high-precision intermediate calculations

### Cross-Program Invocations

- Token program interactions for deposits/withdrawals
- Oracle integration for price feeds
- Potential integrations with Marinade, Solend, Jupiter

## Performance Considerations

### Compute Unit Optimization

- Use lookup tables for expensive calculations (sqrt, tick conversion)
- Minimize account allocations with stack-based data structures
- Batch operations where possible (position management)
- Profile using `cargo bench` in `benches/` directory

### Memory Layout

- Account structures designed for 10KB Solana limit
- Zero-copy patterns to avoid serialization overhead
- Compression techniques for tick storage
- Stack allocation over heap where possible

Remember: This codebase prioritizes mathematical correctness and security over raw performance. Every design decision should consider the economic implications of bugs in DeFi protocols.
