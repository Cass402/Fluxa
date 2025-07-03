use anchor_lang::prelude::*;

pub mod error;
pub mod math;
pub mod utils;

#[cfg(feature = "testing")]
pub mod testing;

declare_id!("11111111111111111111111111111112");

#[program]
pub mod fluxa_core {
    use super::*;

    #[cfg(feature = "testing")]
    pub fn test_fixed_mul_cu(ctx: Context<CuTest>) -> Result<()> {
        testing::cu_tests::test_fixed_mul_cu(ctx)
    }

    #[cfg(feature = "testing")]
    pub fn test_fixed_mul_scenarios(ctx: Context<CuTest>) -> Result<()> {
        testing::cu_tests::test_fixed_mul_scenarios(ctx)
    }
}

#[derive(Accounts)]
pub struct CuTest {}
