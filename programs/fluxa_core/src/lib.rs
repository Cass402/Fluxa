#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

pub mod error;
pub mod math;
pub mod utils;

#[cfg(feature = "cu_testing")]
pub mod cu_tests;

declare_id!("11111111111111111111111111111112");

#[program]
pub mod fluxa_core {

    #[cfg(feature = "cu_testing")]
    pub fn test_basic_arithmetic(ctx: Context<TestBasicArithmetic>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_basic_arithmetic(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_sqrt_variants(ctx: Context<TestSqrtVariants>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_sqrt_variants(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_tick_conversions(ctx: Context<TestTickConversions>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_tick_conversions(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_mul_div_variants(ctx: Context<TestMulDivVariants>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_mul_div_variants(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_liquidity_calculations(ctx: Context<TestLiquidityCalculations>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_liquidity_calculations(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_batch_operations(ctx: Context<TestBatchOperations>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_batch_operations(ctx)
    }

    #[cfg(feature = "cu_testing")]
    pub fn test_edge_cases(ctx: Context<TestEdgeCases>) -> Result<()> {
        crate::cu_tests::core_arithmetic_cu_tests::test_edge_cases(ctx)
    }
}

// Account contexts need to be at the crate root for Anchor to find them
#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestBasicArithmetic {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestSqrtVariants {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestTickConversions {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestMulDivVariants {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestLiquidityCalculations {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestBatchOperations {}

#[cfg(feature = "cu_testing")]
#[derive(Accounts)]
pub struct TestEdgeCases {}
