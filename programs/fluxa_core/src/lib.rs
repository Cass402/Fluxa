#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

pub mod error;
pub mod math;
//pub mod security;
pub mod instructions;
pub mod state;
pub mod utils;

use math::core_arithmetic::{
    liquidity_from_amount_0, liquidity_from_amount_1, mul_div, mul_div_q64, mul_div_round_up,
    recip_q64x64_nearest, sqrt_x64, tick_to_sqrt_x64, Q64x64,
};
use math::liquidity_math::{
    calculate_amount_0_delta, calculate_amount_1_delta, calculate_amounts_for_liquidity_piecewise,
    calculate_liquidity, calculate_position_value_at_price,
};
use state::position::position_account::Position;

declare_id!("CCaEqq6JVbDhCC2ng1UvwvJXkGiKS5Um9G1Y22eeg8tN");

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

    /// Temporary instruction for measuring compute units of `calculate_amount_0_delta`.
    pub fn benchmark_calculate_amount_0_delta(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkCalculateAmountDeltaArgs,
    ) -> Result<()> {
        let sqrt_lower = Q64x64::from_raw(args.sqrt_price_lower);
        let sqrt_upper = Q64x64::from_raw(args.sqrt_price_upper);
        let liquidity = Q64x64::from_raw(args.liquidity);
        let _ = calculate_amount_0_delta(sqrt_lower, sqrt_upper, liquidity)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `calculate_amount_1_delta`.
    pub fn benchmark_calculate_amount_1_delta(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkCalculateAmountDeltaArgs,
    ) -> Result<()> {
        let sqrt_lower = Q64x64::from_raw(args.sqrt_price_lower);
        let sqrt_upper = Q64x64::from_raw(args.sqrt_price_upper);
        let liquidity = Q64x64::from_raw(args.liquidity);
        let _ = calculate_amount_1_delta(sqrt_lower, sqrt_upper, liquidity)?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `calculate_amounts_for_liquidity_piecewise`.
    pub fn benchmark_calculate_amounts_piecewise(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkCalculateAmountsPiecewiseArgs,
    ) -> Result<()> {
        let sqrt_current = Q64x64::from_raw(args.sqrt_price_current);
        let sqrt_lower = Q64x64::from_raw(args.sqrt_price_lower);
        let sqrt_upper = Q64x64::from_raw(args.sqrt_price_upper);
        let liquidity = Q64x64::from_raw(args.liquidity);
        let _ = calculate_amounts_for_liquidity_piecewise(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            liquidity,
        )?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `calculate_liquidity`.
    pub fn benchmark_calculate_liquidity(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkCalculateLiquidityArgs,
    ) -> Result<()> {
        let sqrt_current = Q64x64::from_raw(args.sqrt_price_current);
        let sqrt_lower = Q64x64::from_raw(args.sqrt_price_lower);
        let sqrt_upper = Q64x64::from_raw(args.sqrt_price_upper);
        let _ = calculate_liquidity(
            sqrt_current,
            sqrt_lower,
            sqrt_upper,
            args.amount_0,
            args.amount_1,
        )?;
        Ok(())
    }

    /// Temporary instruction for measuring compute units of `calculate_position_value_at_price`.
    pub fn benchmark_calculate_position_value(
        _ctx: Context<ComputeUnitBenchContext>,
        args: BenchmarkCalculatePositionValueArgs,
    ) -> Result<()> {
        let position = Position {
            owner: args.owner,
            tick_lower: args.tick_lower,
            tick_upper: args.tick_upper,
            status_flags: Position::FLAG_ACTIVE,
            position_nonce: 0,
            _padding1: [0u8; 2],
            liquidity: Q64x64::from_raw(args.liquidity),
            fee_growth_inside_0_last: Q64x64::zero(),
            fee_growth_inside_1_last: Q64x64::zero(),
            tokens_owed_0: Q64x64::zero(),
            tokens_owed_1: Q64x64::zero(),
            total_fees_collected_0: Q64x64::zero(),
            total_fees_collected_1: Q64x64::zero(),
            creation_slot: 0,
            last_update_slot: 0,
            creation_timestamp: 0,
            position_hash: [0u8; 32],
            reserved: [0u64; 3],
        };
        let current_sqrt = Q64x64::from_raw(args.current_sqrt_price);
        let _ = calculate_position_value_at_price(
            &position,
            current_sqrt,
            args.token_0_price_usd,
            args.token_1_price_usd,
        )?;
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkCalculateAmountDeltaArgs {
    pub sqrt_price_lower: u128,
    pub sqrt_price_upper: u128,
    pub liquidity: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkCalculateAmountsPiecewiseArgs {
    pub sqrt_price_current: u128,
    pub sqrt_price_lower: u128,
    pub sqrt_price_upper: u128,
    pub liquidity: u128,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkCalculateLiquidityArgs {
    pub sqrt_price_current: u128,
    pub sqrt_price_lower: u128,
    pub sqrt_price_upper: u128,
    pub amount_0: u64,
    pub amount_1: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct BenchmarkCalculatePositionValueArgs {
    pub owner: Pubkey,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub liquidity: u128,
    pub current_sqrt_price: u128,
    pub token_0_price_usd: u64,
    pub token_1_price_usd: u64,
}
