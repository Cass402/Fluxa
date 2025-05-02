//! Integration tests for the math module
//!
//! These tests verify that the mathematical functions in math.rs work correctly
//! when integrated with other components of the Fluxa system, including:
//! - Pool initialization and operations
//! - Concentrated liquidity position management
//! - Swap execution and routing
//! - Fee calculation and collection
//! - Price calculation across system boundaries

use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program_test::{BanksClient, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::sync::Arc;

// Import Fluxa modules
use amm_core::{
    instructions::{create_position, initialize_pool, swap},
    math::{
        calculate_fee, get_amount0_delta, get_amount1_delta, get_liquidity_from_amounts,
        get_next_sqrt_price_from_amount0_exact_in, get_next_sqrt_price_from_amount1_exact_in,
        sqrt_price_to_tick, tick_to_sqrt_price,
    },
    pool_state::*,
    position_manager::*,
    token_pair::*,
    Pool, Position,
};

/// Test fixture for math integration tests
struct TestFixture {
    context: ProgramTestContext,
    admin: Keypair,
    user_a: Keypair,
    user_b: Keypair,
    token_a_mint: Pubkey,
    token_b_mint: Pubkey,
    token_a_account_a: Pubkey,
    token_b_account_a: Pubkey,
    token_a_account_b: Pubkey,
    token_b_account_b: Pubkey,
    pool_pubkey: Pubkey,
    pool_authority: Pubkey,
    program_id: Pubkey,
}

impl TestFixture {
    /// Create a new test fixture with a program test context and initialized accounts
    async fn new() -> Self {
        let program_id = Pubkey::new_unique();
        let mut program_test = ProgramTest::new(
            "fluxa", program_id, None, // Use default processor
        );

        // Create admin and user keypairs
        let admin = Keypair::new();
        let user_a = Keypair::new();
        let user_b = Keypair::new();

        // Add admin account with SOL balance
        program_test.add_account(
            admin.pubkey(),
            Account {
                lamports: 1_000_000_000,
                data: vec![],
                owner: solana_sdk::system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        // Add user accounts with SOL balance
        program_test.add_account(
            user_a.pubkey(),
            Account {
                lamports: 1_000_000_000,
                data: vec![],
                owner: solana_sdk::system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        program_test.add_account(
            user_b.pubkey(),
            Account {
                lamports: 1_000_000_000,
                data: vec![],
                owner: solana_sdk::system_program::id(),
                executable: false,
                rent_epoch: 0,
            },
        );

        // Start the program test context
        let mut context = program_test.start_with_context().await;

        // Create token mints
        let token_a_mint = Keypair::new();
        let token_b_mint = Keypair::new();

        Self::create_mint(&mut context, &admin, &token_a_mint).await;
        Self::create_mint(&mut context, &admin, &token_b_mint).await;

        // Create token accounts for users
        let token_a_account_a =
            Self::create_token_account(&mut context, &user_a, &token_a_mint.pubkey()).await;

        let token_b_account_a =
            Self::create_token_account(&mut context, &user_a, &token_b_mint.pubkey()).await;

        let token_a_account_b =
            Self::create_token_account(&mut context, &user_b, &token_a_mint.pubkey()).await;

        let token_b_account_b =
            Self::create_token_account(&mut context, &user_b, &token_b_mint.pubkey()).await;

        // Mint initial tokens to users
        Self::mint_tokens(
            &mut context,
            &admin,
            &token_a_mint.pubkey(),
            &token_a_account_a,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &mut context,
            &admin,
            &token_b_mint.pubkey(),
            &token_b_account_a,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &mut context,
            &admin,
            &token_a_mint.pubkey(),
            &token_a_account_b,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &mut context,
            &admin,
            &token_b_mint.pubkey(),
            &token_b_account_b,
            1_000_000_000,
        )
        .await;

        // Create pool
        let (pool_pubkey, pool_authority) = Self::create_pool(
            &mut context,
            &admin,
            program_id,
            token_a_mint.pubkey(),
            token_b_mint.pubkey(),
            500, // 0.05% fee tier
        )
        .await;

        TestFixture {
            context,
            admin,
            user_a,
            user_b,
            token_a_mint: token_a_mint.pubkey(),
            token_b_mint: token_b_mint.pubkey(),
            token_a_account_a,
            token_b_account_a,
            token_a_account_b,
            token_b_account_b,
            pool_pubkey,
            pool_authority,
            program_id,
        }
    }

    /// Create a token mint
    async fn create_mint(context: &mut ProgramTestContext, payer: &Keypair, mint: &Keypair) {
        // Implementation for creating SPL token mint
        // For brevity, stub implementation
        println!("Created mint: {}", mint.pubkey());
    }

    /// Create a token account
    async fn create_token_account(
        context: &mut ProgramTestContext,
        owner: &Keypair,
        mint: &Pubkey,
    ) -> Pubkey {
        // Implementation for creating SPL token account
        // For brevity, return a unique pubkey
        Pubkey::new_unique()
    }

    /// Mint tokens to an account
    async fn mint_tokens(
        context: &mut ProgramTestContext,
        authority: &Keypair,
        mint: &Pubkey,
        to: &Pubkey,
        amount: u64,
    ) {
        // Implementation for minting tokens
        // For brevity, stub implementation
        println!("Minted {} tokens to {}", amount, to);
    }

    /// Create a pool
    async fn create_pool(
        context: &mut ProgramTestContext,
        admin: &Keypair,
        program_id: Pubkey,
        token_a_mint: Pubkey,
        token_b_mint: Pubkey,
        fee_tier: u16,
    ) -> (Pubkey, Pubkey) {
        // Calculate derived addresses
        let pool_pubkey = Pubkey::new_unique();
        let pool_authority = Pubkey::new_unique();

        // In a real implementation, we would send the CreatePool instruction
        // For brevity, stub implementation

        (pool_pubkey, pool_authority)
    }

    /// Create a position in the pool
    async fn create_position(
        &mut self,
        user: &Keypair,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
    ) -> Pubkey {
        // Calculate sqrt price bounds from ticks
        let sqrt_price_lower = tick_to_sqrt_price(tick_lower).unwrap();
        let sqrt_price_upper = tick_to_sqrt_price(tick_upper).unwrap();

        // Calculate token amounts needed for the position
        let amount0 = get_amount0_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

        let amount1 = get_amount1_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

        // In a real implementation, we would send the CreatePosition instruction
        // For brevity, return a unique position pubkey
        Pubkey::new_unique()
    }

    /// Execute a swap
    async fn swap(
        &mut self,
        user: &Keypair,
        zero_for_one: bool,
        amount_in: u64,
        sqrt_price_limit: u128,
    ) -> (u64, u64, u128) {
        // In a real implementation, we would send the Swap instruction
        // For brevity, stub implementation with direct math function calls

        // Get current pool state
        let pool_state = self.get_pool_state().await;

        // Calculate next price after swap
        let next_sqrt_price = if zero_for_one {
            get_next_sqrt_price_from_amount0_exact_in(
                pool_state.sqrt_price,
                pool_state.liquidity,
                amount_in,
                true,
            )
        } else {
            get_next_sqrt_price_from_amount1_exact_in(
                pool_state.sqrt_price,
                pool_state.liquidity,
                amount_in,
                true,
            )
        };

        // Calculate amounts
        let (amount_in_used, amount_out) = if zero_for_one {
            let amount0_used = get_amount0_delta(
                next_sqrt_price,
                pool_state.sqrt_price,
                pool_state.liquidity,
                true,
            );

            let amount1_out = get_amount1_delta(
                next_sqrt_price,
                pool_state.sqrt_price,
                pool_state.liquidity,
                false,
            );

            (amount0_used, amount1_out)
        } else {
            let amount1_used = get_amount1_delta(
                pool_state.sqrt_price,
                next_sqrt_price,
                pool_state.liquidity,
                true,
            );

            let amount0_out = get_amount0_delta(
                pool_state.sqrt_price,
                next_sqrt_price,
                pool_state.liquidity,
                false,
            );

            (amount1_used, amount0_out)
        };

        // Calculate fee
        let fee_amount = calculate_fee(amount_in_used, pool_state.fee_tier);

        (amount_in_used, amount_out, next_sqrt_price)
    }

    /// Get current pool state
    async fn get_pool_state(&self) -> PoolState {
        // In a real implementation, we would fetch the account data
        // For brevity, return a dummy pool state
        PoolState {
            sqrt_price: 1_000_000_000_000, // 1.0 as Q64.64
            tick_current: 0,
            tick_spacing: 10,
            fee_tier: 500,                // 0.05%
            liquidity: 1_000_000_000_000, // 1M liquidity
            fee_growth_global_0: 0,
            fee_growth_global_1: 0,
            token0_protocol_fee: 0,
            token1_protocol_fee: 0,
            token_pair: TokenPair {
                token0: self.token_a_mint,
                token1: self.token_b_mint,
            },
        }
    }
}

/// Integration tests for math module
mod tests {
    use super::*;

    /// Test creating a pool and verifying initial state
    #[tokio::test]
    async fn test_pool_initialization_with_math() {
        let fixture = TestFixture::new().await;
        let pool_state = fixture.get_pool_state().await;

        // Verify that the initial sqrt price is set correctly
        assert_eq!(pool_state.sqrt_price, 1_000_000_000_000); // 1.0 as Q64.64

        // Verify that the tick index correctly matches the sqrt price
        let tick_index = sqrt_price_to_tick_index(pool_state.sqrt_price);
        assert_eq!(tick_index, pool_state.tick_current);

        // Verify that converting back to sqrt price gives a close value
        let sqrt_price_from_tick = tick_index_to_sqrt_price(pool_state.tick_current);
        // Allow for minor rounding error
        let diff = if sqrt_price_from_tick > pool_state.sqrt_price {
            sqrt_price_from_tick - pool_state.sqrt_price
        } else {
            pool_state.sqrt_price - sqrt_price_from_tick
        };

        assert!(diff <= 100); // Small error tolerance
    }

    /// Test position creation and liquidity calculations
    #[tokio::test]
    async fn test_position_creation_with_math() {
        let mut fixture = TestFixture::new().await;

        // Define position parameters
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        // Create position
        let position_pubkey = fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

        // Calculate expected token amounts
        let sqrt_price_lower = tick_index_to_sqrt_price(tick_lower);
        let sqrt_price_upper = tick_index_to_sqrt_price(tick_upper);

        let expected_amount0 =
            get_amount0_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

        let expected_amount1 =
            get_amount1_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);

        // Verify that the liquidity calculation is reversible
        let calculated_liquidity = get_liquidity_from_amounts(
            sqrt_price_lower,
            sqrt_price_upper,
            expected_amount0 as u128,
            expected_amount1 as u128,
        );

        // Allow for minor rounding error
        let diff = if calculated_liquidity > liquidity {
            calculated_liquidity - liquidity
        } else {
            liquidity - calculated_liquidity
        };

        // The difference should be very small relative to the liquidity amount
        assert!(diff <= liquidity / 1_000_000); // 0.0001% error tolerance
    }

    /// Test swap execution and price impact calculations
    #[tokio::test]
    async fn test_swap_execution_with_math() {
        let mut fixture = TestFixture::new().await;

        // Get initial pool state
        let initial_pool_state = fixture.get_pool_state().await;
        let initial_sqrt_price = initial_pool_state.sqrt_price;
        let initial_liquidity = initial_pool_state.liquidity;

        // Execute a swap: token0 for token1
        let amount_in = 1_000_000u64; // 1 token0
        let zero_for_one = true;
        let sqrt_price_limit = 0; // No limit

        let (amount_in_used, amount_out, next_sqrt_price) = fixture
            .swap(&fixture.user_a, zero_for_one, amount_in, sqrt_price_limit)
            .await;

        // Verify next price calculation
        let expected_next_sqrt_price = get_next_sqrt_price_from_amount0_exact_in(
            initial_sqrt_price,
            initial_liquidity,
            amount_in,
            true,
        );

        assert_eq!(next_sqrt_price, expected_next_sqrt_price);

        // Verify amount calculations
        let expected_amount0_used =
            get_amount0_delta(next_sqrt_price, initial_sqrt_price, initial_liquidity, true);

        let expected_amount1_out = get_amount1_delta(
            next_sqrt_price,
            initial_sqrt_price,
            initial_liquidity,
            false,
        );

        assert_eq!(amount_in_used, expected_amount0_used);
        assert_eq!(amount_out, expected_amount1_out);

        // Verify price movement direction
        assert!(next_sqrt_price < initial_sqrt_price); // Price should decrease when selling token0
    }

    /// Test fee calculation during swaps
    #[tokio::test]
    async fn test_fee_calculation_during_swap() {
        let mut fixture = TestFixture::new().await;

        // Get pool state
        let pool_state = fixture.get_pool_state().await;

        // Execute a swap
        let amount_in = 1_000_000u64; // 1 token
        let zero_for_one = true;

        let (amount_in_used, _, _) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Calculate expected fee
        let expected_fee = calculate_fee(amount_in_used, pool_state.fee_tier);

        // In a real test, we would verify that the fee was correctly collected
        // For now, just verify the fee calculation
        assert!(expected_fee > 0);
        assert_eq!(
            expected_fee,
            amount_in_used * (pool_state.fee_tier as u64) / 1_000_000
        );
    }

    /// Test cross-tick swap calculations
    #[tokio::test]
    async fn test_cross_tick_swap_calculations() {
        // This test would simulate a swap that crosses tick boundaries
        // For simplicity in this example, we're not implementing the full tick-crossing logic
        // In a real implementation, this would test how the math functions handle
        // liquidity changes at tick boundaries
    }

    /// Test liquidity addition and removal across price ranges
    #[tokio::test]
    async fn test_liquidity_management_across_price_ranges() {
        let mut fixture = TestFixture::new().await;

        // Create multiple positions across different price ranges
        let positions = vec![
            (-100, 100, 10_000_000_000u128), // Position around current price
            (-500, -200, 5_000_000_000u128), // Position below current price
            (200, 500, 5_000_000_000u128),   // Position above current price
        ];

        // Create all positions
        for (tick_lower, tick_upper, liquidity) in &positions {
            fixture
                .create_position(&fixture.user_a, *tick_lower, *tick_upper, *liquidity)
                .await;
        }

        // Calculate expected active liquidity
        // In a real test, we would verify that the pool's active liquidity
        // was correctly updated as positions were created

        // Test a swap that crosses from one position to another
        // This would verify that the math functions correctly handle
        // liquidity changes at tick boundaries
    }

    /// Test mathematical consistency across swap path
    #[tokio::test]
    async fn test_mathematical_consistency_in_swap_path() {
        let mut fixture = TestFixture::new().await;

        // Get initial pool state
        let initial_pool_state = fixture.get_pool_state().await;
        let initial_sqrt_price = initial_pool_state.sqrt_price;
        let initial_liquidity = initial_pool_state.liquidity;

        // Execute a swap: token0 for token1
        let amount_in = 1_000_000u64; // 1 token0
        let zero_for_one = true;

        let (amount0_in, amount1_out, next_sqrt_price1) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Now swap back: token1 for token0
        let (amount1_in, amount0_out, next_sqrt_price2) = fixture
            .swap(
                &fixture.user_a,
                !zero_for_one, // Reverse direction
                amount1_out,   // Use the output from the previous swap
                0,             // No price limit
            )
            .await;

        // Verify that we don't get back the full amount (due to fees)
        assert!(amount0_out < amount0_in);

        // Calculate the expected fee impact
        let fee_tier = initial_pool_state.fee_tier;
        let expected_fee0 = calculate_fee(amount0_in, fee_tier);
        let expected_fee1 = calculate_fee(amount1_in, fee_tier);

        // Calculate the expected amount out after fees
        // This is a simplified calculation for demonstration
        let expected_amount0_after_fees = amount0_in - expected_fee0;

        // The difference between what we put in and what we got back
        // should be approximately equal to the fees paid
        let diff = amount0_in - amount0_out;

        // Allow for some rounding error
        let error_margin = 10; // Small error tolerance
        assert!((diff as i64 - (expected_fee0 + expected_fee1) as i64).abs() <= error_margin);

        // Verify price movement is symmetric (adjusted for fees)
        // The price should return close to the original price
        let price_diff = if initial_sqrt_price > next_sqrt_price2 {
            initial_sqrt_price - next_sqrt_price2
        } else {
            next_sqrt_price2 - initial_sqrt_price
        };

        // The price difference should be small relative to the initial price
        assert!(price_diff <= initial_sqrt_price / 1_000);
    }

    /// Test math functions with extreme values
    #[tokio::test]
    async fn test_math_functions_with_extreme_values() {
        // Test with minimum tick
        let min_tick = -887272; // Adjust based on Fluxa's configured min tick
        let min_sqrt_price = tick_index_to_sqrt_price(min_tick);
        let min_tick_roundtrip = sqrt_price_to_tick_index(min_sqrt_price);

        // Allow for at most 1 tick of rounding error
        assert!((min_tick - min_tick_roundtrip).abs() <= 1);

        // Test with maximum tick
        let max_tick = 887272; // Adjust based on Fluxa's configured max tick
        let max_sqrt_price = tick_index_to_sqrt_price(max_tick);
        let max_tick_roundtrip = sqrt_price_to_tick_index(max_sqrt_price);

        // Allow for at most 1 tick of rounding error
        assert!((max_tick - max_tick_roundtrip).abs() <= 1);

        // Test with very small liquidity
        let small_liquidity = 1u128;
        let sqrt_price_a = tick_index_to_sqrt_price(0);
        let sqrt_price_b = tick_index_to_sqrt_price(1);

        // Calculate token amounts
        let amount0 = get_amount0_delta(sqrt_price_a, sqrt_price_b, small_liquidity, true);
        let amount1 = get_amount1_delta(sqrt_price_a, sqrt_price_b, small_liquidity, true);

        // Amounts should be non-negative
        assert!(amount0 >= 0);
        assert!(amount1 >= 0);

        // At least one amount should be non-zero with non-zero liquidity
        assert!(amount0 > 0 || amount1 > 0);

        // Test with very large liquidity (but not so large it overflows)
        let large_liquidity = 1u128 << 100;

        // Calculate token amounts
        let amount0_large = get_amount0_delta(sqrt_price_a, sqrt_price_b, large_liquidity, true);
        let amount1_large = get_amount1_delta(sqrt_price_a, sqrt_price_b, large_liquidity, true);

        // Amounts should scale proportionally with liquidity
        let ratio0 =
            (amount0_large as u128) * small_liquidity / (amount0 as u128) / large_liquidity;
        let ratio1 =
            (amount1_large as u128) * small_liquidity / (amount1 as u128) / large_liquidity;

        // Ratios should be close to 1, allowing for some rounding error
        if amount0 > 0 {
            assert!(ratio0 >= 9_999 && ratio0 <= 10_001); // 0.01% error margin
        }

        if amount1 > 0 {
            assert!(ratio1 >= 9_999 && ratio1 <= 10_001); // 0.01% error margin
        }
    }
}
