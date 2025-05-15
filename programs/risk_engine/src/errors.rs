use anchor_lang::prelude::*;

#[error_code]
pub enum RiskEngineError {
    #[msg("Price data from oracle is too stale.")]
    OraclePriceStale,
    #[msg("Volatility data unavailable or insufficient.")]
    VolatilityDataError,
    #[msg("Optimization failed to find better parameters.")]
    OptimizationFailed,
    #[msg("Proposed rebalance is not beneficial under current conditions.")]
    RebalanceNotBeneficialMvp,
    #[msg("AMM core account mismatch or invalid.")]
    InvalidAmmCoreAccount,
    #[msg("Position not found or not owned by caller.")]
    PositionAccessDenied,
    #[msg("Calculation error, possibly due to zero denominator.")]
    CalculationError,
    #[msg("Overflow in calculation")]
    Overflow,
}
