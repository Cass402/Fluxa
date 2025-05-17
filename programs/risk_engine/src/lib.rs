#![allow(unexpected_cfgs)]
use amm_core::position::PositionData as AmmPositionData;
use amm_core::program::AmmCore; // To CPI to amm_core
use amm_core::state::pool::Pool as AmmPool;
use anchor_lang::prelude::*;
// use amm_core::tick::TickData as AmmTickData; // For CPI context if needed
use amm_core::cpi;
use amm_core::cpi::accounts::UpdatePosition as AmmUpdatePositionCtx; // For CPI // For cpi::update_position_handler

pub mod errors;
pub mod il_analyzer;
pub mod position_optimizer;
pub mod volatility_detector;

use errors::RiskEngineError;
// Use the isqrt function from volatility_detector
use volatility_detector::isqrt_u128;

/// Placeholder for price precision, e.g., 10^6 for 6 decimal places.
const PRICE_SCALE_FACTOR: u128 = 1_000_000; // 6 decimal places

declare_id!("6wVb2AKyTcGE3x2xFjpPaDR1CE3q8LZZkHx3JvYrKNoa"); // Replace with your actual Program ID

#[program]
pub mod fluxa_risk_engine {
    use super::*;

    pub fn trigger_rebalance_check(
        ctx: Context<TriggerRebalanceCheck>,
        // We might need position_entry_sqrt_price if not stored in AmmPositionData
        // For MVP, assume it's derivable or we use a fixed one for demo.
        // For a real system, this would be tracked.
        position_entry_sqrt_price_q64: u128,
    ) -> Result<()> {
        let amm_position = &ctx.accounts.amm_position;
        let amm_pool = &ctx.accounts.amm_pool;

        // --- 1. Get Data ---
        // For MVP, assume price history comes from oracle or is simulated for volatility.
        // Let's use a placeholder for price history for the volatility calculation.
        // Prices are scaled by PRICE_SCALE_FACTOR.
        let placeholder_price_history: Vec<u128> = vec![
            100 * PRICE_SCALE_FACTOR,
            101 * PRICE_SCALE_FACTOR,
            100 * PRICE_SCALE_FACTOR + 500_000, // 100.5
            102 * PRICE_SCALE_FACTOR,
            101 * PRICE_SCALE_FACTOR + 500_000, // 101.5
            103 * PRICE_SCALE_FACTOR,
            102 * PRICE_SCALE_FACTOR + 500_000, // 102.5
            104 * PRICE_SCALE_FACTOR,
            103 * PRICE_SCALE_FACTOR + 500_000, // 103.5
            105 * PRICE_SCALE_FACTOR,
            104 * PRICE_SCALE_FACTOR + 500_000, // 104.5
            106 * PRICE_SCALE_FACTOR,
            105 * PRICE_SCALE_FACTOR + 500_000, // 105.5
            107 * PRICE_SCALE_FACTOR,
            106 * PRICE_SCALE_FACTOR + 500_000, // 106.5
            108 * PRICE_SCALE_FACTOR,
            107 * PRICE_SCALE_FACTOR + 500_000, // 107.5
            109 * PRICE_SCALE_FACTOR,
            108 * PRICE_SCALE_FACTOR + 500_000, // 108.5
            110 * PRICE_SCALE_FACTOR,
        ]; // Needs at least `window_size` elements
        let current_sqrt_price_q64 = amm_pool.sqrt_price_q64; // From the AMM pool state

        // --- 2. Volatility Detection (Simplified) ---
        let window_size = 10; // Example window size
        let daily_volatility_scaled = volatility_detector::calculate_rolling_std_dev_volatility(
            &placeholder_price_history, // Replace with actual price data source
            window_size,
        )?;
        // daily_volatility_scaled is scaled by volatility_detector::RETURN_SCALING_FACTOR

        // Convert to annualized: annualized_vol = daily_vol * sqrt(365)
        // All calculations in fixed point.
        const DAYS_IN_YEAR_U128: u128 = 365;
        // Using a precision scale for sqrt calculation intermediate step
        const SQRT_PRECISION_SCALE: u128 = 1_000_000_000; // 10^9 for sqrt precision

        let sqrt_365_scaled_for_calc =
            isqrt_u128(DAYS_IN_YEAR_U128 * SQRT_PRECISION_SCALE * SQRT_PRECISION_SCALE);

        // annualized_volatility_scaled will have the same scale as daily_volatility_scaled
        // (i.e., volatility_detector::RETURN_SCALING_FACTOR)
        let annualized_volatility_scaled =
            (daily_volatility_scaled * sqrt_365_scaled_for_calc) / SQRT_PRECISION_SCALE;

        msg!(
            "Calculated Volatility (annualized, scaled by {}): {}",
            volatility_detector::RETURN_SCALING_FACTOR,
            annualized_volatility_scaled
        );

        // --- 3. IL Analysis (Basic) ---
        let il_percentage = il_analyzer::calculate_current_il_percentage(
            amm_position.tick_lower_index,
            amm_position.tick_upper_index,
            position_entry_sqrt_price_q64, // Sqrt price when position was opened
            current_sqrt_price_q64,
        )?;
        // il_percentage is an i128 scaled by il_analyzer::IL_PERCENTAGE_SCALE
        msg!(
            "Current IL Percentage (scaled by {}): {}",
            il_analyzer::IL_PERCENTAGE_SCALE,
            il_percentage
        );

        // --- 4. Position Optimization (Simplified) ---
        let (new_lower_tick, new_upper_tick) =
            position_optimizer::calculate_optimal_boundaries_mvp(
                current_sqrt_price_q64,
                annualized_volatility_scaled, // Pass annualized volatility, scaled by VOLATILITY_INPUT_SCALE
                amm_pool.tick_spacing,
            )?;
        msg!(
            "Proposed new boundaries: Lower Tick {}, Upper Tick {}",
            new_lower_tick,
            new_upper_tick
        );

        // --- 5. Rebalance Decision (MVP: Rebalance if different and IL is negative) ---
        let old_lower_tick = amm_position.tick_lower_index;
        let old_upper_tick = amm_position.tick_upper_index;

        if new_lower_tick != old_lower_tick || new_upper_tick != old_upper_tick {
            // For MVP, let's add a simple condition, e.g. rebalance if IL is negative.
            // A real system would have a much more sophisticated cost/benefit analysis.
            // -0.01% IL threshold, scaled:
            // -0.01 / 100 * IL_PERCENTAGE_SCALE = -(IL_PERCENTAGE_SCALE / 10_000)
            let il_threshold_scaled: i128 = -((il_analyzer::IL_PERCENTAGE_SCALE as i128) / 10_000);

            if il_percentage < il_threshold_scaled {
                msg!(
                    "Rebalancing conditions met. IL (scaled by {}): {}, New Ticks: [{}, {}]",
                    il_analyzer::IL_PERCENTAGE_SCALE,
                    il_percentage,
                    new_lower_tick,
                    new_upper_tick
                );

                // --- 6. CPI to amm_core to update position ---
                let cpi_program = ctx.accounts.amm_core_program.to_account_info();
                let cpi_accounts = AmmUpdatePositionCtx {
                    pool: ctx.accounts.amm_pool.to_account_info(),
                    position: ctx.accounts.amm_position.to_account_info(),
                    old_tick_lower: ctx.accounts.amm_old_tick_lower.to_account_info(),
                    old_tick_upper: ctx.accounts.amm_old_tick_upper.to_account_info(),
                    new_tick_lower: ctx.accounts.amm_new_tick_lower.to_account_info(),
                    new_tick_upper: ctx.accounts.amm_new_tick_upper.to_account_info(),
                    owner: ctx.accounts.owner.to_account_info(), // Risk engine is the authority
                    payer: ctx.accounts.payer.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                };

                // Derive PDA signer seeds if risk engine is the authority
                // For MVP, owner is signer, so no PDA seeds needed here for CPI authority.

                cpi::update_position_handler(
                    CpiContext::new(cpi_program, cpi_accounts),
                    new_lower_tick,
                    new_upper_tick,
                )?;
                msg!("Position rebalanced in AMM Core.");
            } else {
                msg!(
                    "Rebalance not beneficial or IL not significant enough for MVP. IL (scaled by {}): {}",
                    il_analyzer::IL_PERCENTAGE_SCALE, il_percentage
                );
                return Err(RiskEngineError::RebalanceNotBeneficialMvp.into());
            }
        } else {
            msg!("No change in optimal boundaries. No rebalance needed.");
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TriggerRebalanceCheck<'info> {
    // AMM Core accounts
    #[account(mut, constraint = amm_pool.key() == amm_position.pool @ RiskEngineError::InvalidAmmCoreAccount)]
    pub amm_pool: Account<'info, AmmPool>, // From amm_core crate

    #[account(mut)] // Position data from amm_core, needs to be mutable for CPI
    pub amm_position: Account<'info, AmmPositionData>,

    // Tick accounts for AMM Core CPI call. These need to be passed by the client.
    // The client needs to know/derive the PDAs for these based on the *current*
    // ticks of the amm_position, and the *new* ticks proposed by the optimizer.
    /// CHECK: Account for old_tick_lower, validated by CPI to amm_core
    #[account(mut)]
    pub amm_old_tick_lower: UncheckedAccount<'info>,
    /// CHECK: Account for old_tick_upper, validated by CPI to amm_core
    #[account(mut)]
    pub amm_old_tick_upper: UncheckedAccount<'info>,
    /// CHECK: Account for new_tick_lower, validated by CPI to amm_core
    #[account(mut)]
    pub amm_new_tick_lower: UncheckedAccount<'info>,
    /// CHECK: Account for new_tick_upper, validated by CPI to amm_core
    #[account(mut)]
    pub amm_new_tick_upper: UncheckedAccount<'info>,

    // Oracle account (e.g., Pyth price feed)
    // For MVP, this might be simplified or data passed directly.
    // If used, ensure it's properly constrained (e.g., correct feed for the pool's tokens)
    // pub pyth_price_feed: Account<'info, pyth_sdk_solana::Price>,

    // Signer & Payer
    // For MVP, the position owner might be the one signing to trigger this.
    // In a more automated system, this could be a keeper bot or the risk engine's PDA.
    #[account(mut, address = amm_position.owner @ RiskEngineError::PositionAccessDenied)]
    // Ensure signer is the position owner
    pub owner: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>, // To pay for CPI and potentially new tick accounts in AMM

    // Programs
    pub amm_core_program: Program<'info, AmmCore>, // CPI to amm_core
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
