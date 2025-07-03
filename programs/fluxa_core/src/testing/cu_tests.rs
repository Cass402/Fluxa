use crate::math::fixed_point::core_arithmetic::Fixed;
use crate::CuTest;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;

pub fn test_fixed_mul_cu(_ctx: Context<CuTest>) -> Result<()> {
    msg!("=== Fixed Point Multiplication CU Test ===");

    sol_log_compute_units();

    let a = Fixed::<18>::from_integer(100)?;
    let b = Fixed::<18>::from_integer(200)?;

    msg!(
        "Before multiplication: a={}, b={}",
        a.raw_value(),
        b.raw_value()
    );

    let result = a * b;

    sol_log_compute_units();

    match result {
        Ok(val) => msg!("Multiplication result: {}", val.raw_value()),
        Err(_) => msg!("Multiplication failed"),
    }

    Ok(())
}

pub fn test_fixed_mul_scenarios(_ctx: Context<CuTest>) -> Result<()> {
    msg!("=== Testing Multiple Precision Levels ===");

    // Test PRECISION = 8
    msg!("Testing PRECISION=8");
    sol_log_compute_units();
    let small_8 = Fixed::<8>::from_integer(10)?;
    let large_8 = Fixed::<8>::from_integer(1000)?;
    let _r1 = small_8 * small_8;
    let _r2 = large_8 * large_8;
    let _r3 = small_8 * large_8;
    sol_log_compute_units();

    // Test PRECISION = 18
    msg!("Testing PRECISION=18");
    sol_log_compute_units();
    let small_18 = Fixed::<18>::from_integer(10)?;
    let large_18 = Fixed::<18>::from_integer(1000)?;
    let _r1 = small_18 * small_18;
    let _r2 = large_18 * large_18;
    let _r3 = small_18 * large_18;
    sol_log_compute_units();

    // Test PRECISION = 32
    msg!("Testing PRECISION=32");
    sol_log_compute_units();
    let small_32 = Fixed::<32>::from_integer(10)?;
    let large_32 = Fixed::<32>::from_integer(1000)?;
    let _r1 = small_32 * small_32;
    let _r2 = large_32 * large_32;
    let _r3 = small_32 * large_32;
    sol_log_compute_units();

    Ok(())
}
