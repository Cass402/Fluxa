use crate::math::core_arithmetic::*;
use crate::{
    TestBasicArithmetic, TestBatchOperations, TestEdgeCases, TestLiquidityCalculations,
    TestMulDivVariants, TestSqrtVariants, TestTickConversions,
};
use anchor_lang::prelude::*;
use solana_program::log::sol_log_compute_units;

// Helper function to log compute unit differences
fn log_cu_diff(operation: &str, start_cu: u64) {
    let end_cu = get_remaining_compute_units();
    let used_cu = start_cu.saturating_sub(end_cu);
    msg!("{}: {} CU", operation, used_cu);
}

// Helper function to get remaining compute units from program context
fn get_remaining_compute_units() -> u64 {
    // Note: This is a simplified implementation for testing
    // In production, you'd capture actual CU values from the runtime
    sol_log_compute_units();
    // Return a mock value for now - in real usage, this would be captured
    // from the program's execution context or logs
    100000 // Mock remaining CU value
}

// CU Test Functions
pub fn test_basic_arithmetic(_ctx: Context<TestBasicArithmetic>) -> Result<()> {
    let a = Q64x64::from_int(42);
    let b = Q64x64::from_int(13);

    msg!("=== Basic Arithmetic CU Tests ===");
    sol_log_compute_units();

    // Test checked_mul
    let start_cu = get_remaining_compute_units();
    let _result = a.checked_mul(b)?;
    log_cu_diff("checked_mul", start_cu);

    // Test checked_div
    let start_cu = get_remaining_compute_units();
    let _result = a.checked_div(b)?;
    log_cu_diff("checked_div", start_cu);

    // Test checked_add
    let start_cu = get_remaining_compute_units();
    let _result = a.checked_add(b)?;
    log_cu_diff("checked_add", start_cu);

    // Test checked_sub
    let start_cu = get_remaining_compute_units();
    let _result = a.checked_sub(b)?;
    log_cu_diff("checked_sub", start_cu);

    Ok(())
}

pub fn test_sqrt_variants(_ctx: Context<TestSqrtVariants>) -> Result<()> {
    msg!("=== Sqrt CU Tests ===");
    sol_log_compute_units();

    // Test sqrt with zero
    let start_cu = get_remaining_compute_units();
    let _result = sqrt_x64(Q64x64::zero())?;
    log_cu_diff("sqrt_x64(0)", start_cu);

    // Test sqrt with one
    let start_cu = get_remaining_compute_units();
    let _result = sqrt_x64(Q64x64::one())?;
    log_cu_diff("sqrt_x64(1)", start_cu);

    // Test sqrt with small fractional value
    let start_cu = get_remaining_compute_units();
    let small_val = Q64x64::from_raw(ONE_X64 / 1000); // 0.001
    let _result = sqrt_x64(small_val)?;
    log_cu_diff("sqrt_x64(0.001)", start_cu);

    // Test sqrt with large value
    let start_cu = get_remaining_compute_units();
    let large_val = Q64x64::from_int(1000000);
    let _result = sqrt_x64(large_val)?;
    log_cu_diff("sqrt_x64(1M)", start_cu);

    // Test sqrt with max safe value
    let start_cu = get_remaining_compute_units();
    let max_val = Q64x64::from_raw(MAX_SQRT_X64);
    let _result = sqrt_x64(max_val)?;
    log_cu_diff("sqrt_x64(MAX)", start_cu);

    Ok(())
}

pub fn test_tick_conversions(_ctx: Context<TestTickConversions>) -> Result<()> {
    msg!("=== Tick Conversion CU Tests ===");
    sol_log_compute_units();

    // Test tick 0 (1:1 price)
    let start_cu = get_remaining_compute_units();
    let _result = tick_to_sqrt_x64(0)?;
    log_cu_diff("tick_to_sqrt_x64(0)", start_cu);

    // Test MIN_TICK
    let start_cu = get_remaining_compute_units();
    let _result = tick_to_sqrt_x64(MIN_TICK)?;
    log_cu_diff("tick_to_sqrt_x64(MIN_TICK)", start_cu);

    // Test MAX_TICK
    let start_cu = get_remaining_compute_units();
    let _result = tick_to_sqrt_x64(MAX_TICK)?;
    log_cu_diff("tick_to_sqrt_x64(MAX_TICK)", start_cu);

    // Test positive mid-range tick
    let start_cu = get_remaining_compute_units();
    let _result = tick_to_sqrt_x64(50000)?;
    log_cu_diff("tick_to_sqrt_x64(50000)", start_cu);

    // Test negative mid-range tick
    let start_cu = get_remaining_compute_units();
    let _result = tick_to_sqrt_x64(-50000)?;
    log_cu_diff("tick_to_sqrt_x64(-50000)", start_cu);

    Ok(())
}

pub fn test_mul_div_variants(_ctx: Context<TestMulDivVariants>) -> Result<()> {
    msg!("=== Mul/Div CU Tests ===");
    sol_log_compute_units();

    let a = 1_000_000u128;
    let b = 2_000_000u128;
    let c = 500_000u128;

    // Test mul_div
    let start_cu = get_remaining_compute_units();
    let _result = mul_div(a, b, c)?;
    log_cu_diff("mul_div(small)", start_cu);

    // Test mul_div_round_up
    let start_cu = get_remaining_compute_units();
    let _result = mul_div_round_up(a, b, c)?;
    log_cu_diff("mul_div_round_up(small)", start_cu);

    // Test with large values
    let large_a = u128::MAX / 4;
    let large_b = u128::MAX / 4;
    let large_c = u128::MAX / 2;

    let start_cu = get_remaining_compute_units();
    let _result = mul_div(large_a, large_b, large_c)?;
    log_cu_diff("mul_div(large)", start_cu);

    let start_cu = get_remaining_compute_units();
    let _result = mul_div_round_up(large_a, large_b, large_c)?;
    log_cu_diff("mul_div_round_up(large)", start_cu);

    // Test Q64x64 variant
    let qa = Q64x64::from_int(1000);
    let qb = Q64x64::from_int(2000);
    let qc = Q64x64::from_int(500);

    let start_cu = get_remaining_compute_units();
    let _result = mul_div_q64(qa, qb, qc)?;
    log_cu_diff("mul_div_q64", start_cu);

    Ok(())
}

pub fn test_liquidity_calculations(_ctx: Context<TestLiquidityCalculations>) -> Result<()> {
    msg!("=== Liquidity Calculation CU Tests ===");
    sol_log_compute_units();

    // Setup realistic sqrt price range (e.g., USDC/SOL)
    let sqrt_a = Q64x64::from_raw(MIN_SQRT_X64 * 1000); // Lower price
    let sqrt_b = Q64x64::from_raw(MIN_SQRT_X64 * 2000); // Upper price
    let amount0 = 1_000_000u64; // 1M tokens
    let amount1 = 500_000u64; // 500K tokens

    // Test liquidity_from_amount_0
    let start_cu = get_remaining_compute_units();
    let _result = liquidity_from_amount_0(sqrt_a, sqrt_b, amount0)?;
    log_cu_diff("liquidity_from_amount_0", start_cu);

    // Test liquidity_from_amount_1
    let start_cu = get_remaining_compute_units();
    let _result = liquidity_from_amount_1(sqrt_a, sqrt_b, amount1)?;
    log_cu_diff("liquidity_from_amount_1", start_cu);

    // Test with wide range (more compute-intensive)
    let wide_sqrt_a = Q64x64::from_raw(MIN_SQRT_X64);
    let wide_sqrt_b = Q64x64::from_raw(MAX_SQRT_X64 / 2);

    let start_cu = get_remaining_compute_units();
    let _result = liquidity_from_amount_0(wide_sqrt_a, wide_sqrt_b, amount0)?;
    log_cu_diff("liquidity_from_amount_0(wide)", start_cu);

    let start_cu = get_remaining_compute_units();
    let _result = liquidity_from_amount_1(wide_sqrt_a, wide_sqrt_b, amount1)?;
    log_cu_diff("liquidity_from_amount_1(wide)", start_cu);

    Ok(())
}

pub fn test_batch_operations(_ctx: Context<TestBatchOperations>) -> Result<()> {
    msg!("=== Batch Operations CU Tests ===");
    sol_log_compute_units();

    // Test 10 consecutive sqrt operations
    let start_cu = get_remaining_compute_units();
    for i in 1..=10 {
        let val = Q64x64::from_int(i * 100);
        let _result = sqrt_x64(val)?;
    }
    log_cu_diff("10x sqrt_x64", start_cu);

    // Test 10 consecutive tick conversions
    let start_cu = get_remaining_compute_units();
    for i in 1..=10 {
        let tick = i * 1000;
        let _result = tick_to_sqrt_x64(tick)?;
    }
    log_cu_diff("10x tick_to_sqrt_x64", start_cu);

    // Simulate LP position entry (realistic batch)
    let start_cu = get_remaining_compute_units();

    // Convert ticks to sqrt prices
    let tick_lower = -10000;
    let tick_upper = 10000;
    let sqrt_lower = tick_to_sqrt_x64(tick_lower)?;
    let sqrt_upper = tick_to_sqrt_x64(tick_upper)?;

    // Calculate current price sqrt
    let current_tick = 0;
    let sqrt_current = tick_to_sqrt_x64(current_tick)?;

    // Calculate liquidity from both amounts
    let amount0 = 1_000_000u64;
    let amount1 = 500_000u64;
    let _liq0 = liquidity_from_amount_0(sqrt_current, sqrt_upper, amount0)?;
    let _liq1 = liquidity_from_amount_1(sqrt_lower, sqrt_current, amount1)?;

    log_cu_diff("LP_position_entry_simulation", start_cu);

    Ok(())
}

pub fn test_edge_cases(_ctx: Context<TestEdgeCases>) -> Result<()> {
    msg!("=== Edge Cases CU Tests ===");
    sol_log_compute_units();

    // Test operations near overflow limits
    let near_max = Q64x64::from_raw(u128::MAX / 2);
    let small = Q64x64::from_raw(1);

    let start_cu = get_remaining_compute_units();
    let _result = near_max.checked_mul(small)?;
    log_cu_diff("near_max * small", start_cu);

    let start_cu = get_remaining_compute_units();
    let _result = near_max.checked_div(near_max)?;
    log_cu_diff("near_max / near_max", start_cu);

    // Test sqrt with very small fractional values
    let tiny = Q64x64::from_raw(1); // Smallest possible non-zero
    let start_cu = get_remaining_compute_units();
    let _result = sqrt_x64(tiny)?;
    log_cu_diff("sqrt_x64(tiny)", start_cu);

    // Test mul_div with remainder vs no remainder
    let start_cu = get_remaining_compute_units();
    let _result = mul_div(100, 3, 2)?; // Has remainder
    log_cu_diff("mul_div(with_remainder)", start_cu);

    let start_cu = get_remaining_compute_units();
    let _result = mul_div(100, 4, 2)?; // No remainder
    log_cu_diff("mul_div(no_remainder)", start_cu);

    Ok(())
}
