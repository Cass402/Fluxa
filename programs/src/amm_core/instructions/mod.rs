/// # Fluxa AMM Core Instructions
///
/// This module contains all instruction handlers for the Fluxa AMM Core program,
/// providing the essential functionality for a concentrated liquidity automated market maker.
///
/// ## Instruction Flow and Architecture
///
/// The instructions in this module are generally executed in the following order:
///
/// 1. **Token Pair Creation** - Create a new trading pair between two tokens
/// 2. **Pool Initialization** - Initialize a pool with a specific fee tier for a token pair
/// 3. **Position Creation** - Create liquidity positions within specific price ranges
/// 4. **Trading** - Execute swaps between tokens using the provided liquidity
/// 5. **Fee Collection** - Collect accumulated fees from positions
/// 6. **Position Modification** - Increase or decrease liquidity in existing positions
///
/// Each instruction is isolated in its own module with clear documentation on
/// the accounts required and the constraints that must be satisfied.
///
/// ## Security Considerations
///
/// The instruction handlers incorporate multiple security checks:
///
/// - Owner verification for operations on positions
/// - Proper accounting of pool liquidity and position balances
/// - Validated token transfers with appropriate signers
/// - Price slippage protection for trades
///
/// ## Fee Structure
///
/// Fluxa AMM supports multiple fee tiers (similar to Uniswap v3):
/// - Low: For stable pairs with minimal price movements
/// - Medium: For standard pairs with moderate volatility
/// - High: For exotic pairs with high volatility
///
/// A portion of all fees (the protocol fee) is reserved for protocol governance,
/// while the majority goes to liquidity providers proportional to their contribution.
pub mod collect_fees;
pub mod create_position;
pub mod create_token_pair;
pub mod initialize_pool;
pub mod modify_position;
pub mod swap;
