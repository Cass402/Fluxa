#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

pub mod error;
pub mod math;
pub mod security;
pub mod state;
pub mod utils;

use math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, mul_div, mul_div_q64, mul_div_round_up,
    recip_q64x64_nearest, sqrt_x64, tick_to_sqrt_x64, Q64x64,
};

declare_id!("4i7vUz8hdydUDGqY2AiabhVJSQZvzDRbSB4kcypnjcWp");

#[program]
pub mod fluxa_core {
    use super::*;

    /// Temporary instruction for measuring compute units of `mul_div`.
    pub fn benchmark_mul_div(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkMulDivArgs,
    ) -> Result<()> {
        let _ = mul_div(args.a, args.b, args.c)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `mul_div_round_up`.
    pub fn benchmark_mul_div_round_up(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkMulDivArgs,
    ) -> Result<()> {
        let _ = mul_div_round_up(args.a, args.b, args.c)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `mul_div_q64`.
    pub fn benchmark_mul_div_q64(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkMulDivQ64Args,
    ) -> Result<()> {
        let a = Q64x64::from_raw(args.a_raw);
        let b = Q64x64::from_raw(args.b_raw);
        let c = Q64x64::from_raw(args.c_raw);
        let _ = mul_div_q64(a, b, c)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `sqrt_x64`.
    pub fn benchmark_sqrt_x64(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkSqrtX64Args,
    ) -> Result<()> {
        let value = Q64x64::from_raw(args.value);
        let _ = sqrt_x64(value)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `recip_q64x64_nearest`.
    pub fn benchmark_recip_q64x64_nearest(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkRecipArgs,
    ) -> Result<()> {
        let _ = recip_q64x64_nearest(args.raw);
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `tick_to_sqrt_x64`.
    pub fn benchmark_tick_to_sqrt_x64(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkTickToSqrtArgs,
    ) -> Result<()> {
        let _ = tick_to_sqrt_x64(args.tick)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `liquidity_from_amount_0`.
    pub fn benchmark_liquidity_from_amount_0(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkLiquidityFromAmount0Args,
    ) -> Result<()> {
        let sqrt_a = Q64x64::from_raw(args.sqrt_a);
        let sqrt_b = Q64x64::from_raw(args.sqrt_b);
        let _ = liquidity_from_amount_0(sqrt_a, sqrt_b, args.amount_0)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `liquidity_from_amount_1`.
    pub fn benchmark_liquidity_from_amount_1(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkLiquidityFromAmount1Args,
    ) -> Result<()> {
        let sqrt_a = Q64x64::from_raw(args.sqrt_a);
        let sqrt_b = Q64x64::from_raw(args.sqrt_b);
        let _ = liquidity_from_amount_1(sqrt_a, sqrt_b, args.amount_1)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ComputeUnitBenchContext<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkMulDivArgs {
    pub a: u128,
    pub b: u128,
    pub c: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkMulDivQ64Args {
    pub a_raw: u128,
    pub b_raw: u128,
    pub c_raw: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkSqrtX64Args {
    pub value: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkRecipArgs {
    pub raw: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkTickToSqrtArgs {
    pub tick: i32,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkLiquidityFromAmount0Args {
    pub sqrt_a: u128,
    pub sqrt_b: u128,
    pub amount_0: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkLiquidityFromAmount1Args {
    pub sqrt_a: u128,
    pub sqrt_b: u128,
    pub amount_1: u64,
}
