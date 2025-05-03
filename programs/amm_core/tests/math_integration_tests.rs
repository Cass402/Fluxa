//! Integration tests for the math module
//!
//! These tests verify that the mathematical functions in math.rs work correctly
//! when integrated with other components of the Fluxa system, including:
//! - Pool initialization and operations
//! - Concentrated liquidity position management
//! - Swap execution and routing
//! - Fee calculation and collection
//! - Price calculation across system boundaries

use std::str::FromStr;
use std::sync::Arc;

// Anchor imports
use anchor_client::{
    solana_sdk::{
        commitment_config::CommitmentConfig,
        pubkey::Pubkey,
        rent::Rent,
        signature::{Keypair, Signer},
        system_instruction, sysvar,
    },
    Client, Cluster, Program,
};
use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use spl_token::instruction as token_instruction;

// Use Anchor's test framework
use anchor_lang::solana_program::program_pack::Pack;
use anchor_lang::InstructionData;
use anchor_lang::ToAccountMetas;

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
    Pool, Position, ID as FLUXA_PROGRAM_ID,
};

// Import Anchor test utilities
#[cfg(test)]
use anchor_client::ClientError;

/// Test fixture for math integration tests using Anchor's framework
struct TestFixture {
    // Anchor client
    client: Client<Arc<Keypair>>,
    program: Program<Arc<Keypair>>,

    // Test accounts
    admin: Keypair,
    user_a: Keypair,
    user_b: Keypair,

    // Token mints and accounts
    token_a_mint: Pubkey,
    token_b_mint: Pubkey,
    token_a_account_a: Pubkey,
    token_b_account_a: Pubkey,
    token_a_account_b: Pubkey,
    token_b_account_b: Pubkey,

    // Pool data
    pool_pubkey: Pubkey,
    pool_authority: Pubkey,
}

impl TestFixture {
    /// Create a new test fixture with Anchor framework
    async fn new() -> Self {
        // Use the local validator
        let url = Cluster::Localnet;

        // Create admin and user keypairs
        let admin = Keypair::new();
        let user_a = Keypair::new();
        let user_b = Keypair::new();

        // Create Anchor client with admin as payer
        let client =
            Client::new_with_options(url, Arc::new(Keypair::new()), CommitmentConfig::processed());

        // Get program from client
        let program = client
            .program(FLUXA_PROGRAM_ID)
            .expect("Failed to get program");

        // Airdrop SOL to accounts
        Self::airdrop_sol(&program, &admin.pubkey(), 100_000_000_000).await;
        Self::airdrop_sol(&program, &user_a.pubkey(), 10_000_000_000).await;
        Self::airdrop_sol(&program, &user_b.pubkey(), 10_000_000_000).await;

        // Create token mints
        let token_a_mint = Self::create_mint(&program, &admin).await;
        let token_b_mint = Self::create_mint(&program, &admin).await;

        // Create token accounts
        let token_a_account_a =
            Self::create_token_account(&program, &admin, &token_a_mint, &user_a.pubkey()).await;

        let token_b_account_a =
            Self::create_token_account(&program, &admin, &token_b_mint, &user_a.pubkey()).await;

        let token_a_account_b =
            Self::create_token_account(&program, &admin, &token_a_mint, &user_b.pubkey()).await;

        let token_b_account_b =
            Self::create_token_account(&program, &admin, &token_b_mint, &user_b.pubkey()).await;

        // Mint tokens to users
        Self::mint_tokens(
            &program,
            &admin,
            &token_a_mint,
            &token_a_account_a,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &program,
            &admin,
            &token_b_mint,
            &token_b_account_a,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &program,
            &admin,
            &token_a_mint,
            &token_a_account_b,
            1_000_000_000,
        )
        .await;

        Self::mint_tokens(
            &program,
            &admin,
            &token_b_mint,
            &token_b_account_b,
            1_000_000_000,
        )
        .await;

        // Create pool
        let (pool_pubkey, pool_authority) = Self::create_pool(
            &program,
            &admin,
            token_a_mint,
            token_b_mint,
            500, // 0.05% fee tier
        )
        .await;

        TestFixture {
            client,
            program,
            admin,
            user_a,
            user_b,
            token_a_mint,
            token_b_mint,
            token_a_account_a,
            token_b_account_a,
            token_a_account_b,
            token_b_account_b,
            pool_pubkey,
            pool_authority,
        }
    }

    /// Airdrop SOL to an account
    async fn airdrop_sol(program: &Program<Arc<Keypair>>, recipient: &Pubkey, amount: u64) {
        program.rpc().request_airdrop(recipient, amount).unwrap();
    }

    /// Create a token mint
    async fn create_mint(program: &Program<Arc<Keypair>>, payer: &Keypair) -> Pubkey {
        let mint = Keypair::new();

        // Create mint account
        let create_mint_instr = system_instruction::create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            Rent::default().minimum_balance(Mint::LEN),
            Mint::LEN as u64,
            &token::ID,
        );

        // Initialize mint
        let init_mint_instr = token_instruction::initialize_mint(
            &token::ID,
            &mint.pubkey(),
            &payer.pubkey(),
            None,
            9, // 9 decimals
        )
        .unwrap();

        // Build and send transaction
        let recent_blockhash = program.rpc().get_latest_blockhash().unwrap();
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[create_mint_instr, init_mint_instr],
            Some(&payer.pubkey()),
            &[payer, &mint],
            recent_blockhash,
        );

        program
            .rpc()
            .send_and_confirm_transaction(&transaction)
            .unwrap();

        mint.pubkey()
    }

    /// Create a token account
    async fn create_token_account(
        program: &Program<Arc<Keypair>>,
        payer: &Keypair,
        mint: &Pubkey,
        owner: &Pubkey,
    ) -> Pubkey {
        let token_account = Keypair::new();

        // Create account
        let create_account_instr = system_instruction::create_account(
            &payer.pubkey(),
            &token_account.pubkey(),
            Rent::default().minimum_balance(anchor_spl::token::TokenAccount::LEN),
            anchor_spl::token::TokenAccount::LEN as u64,
            &token::ID,
        );

        // Initialize token account
        let init_account_instr =
            token_instruction::initialize_account(&token::ID, &token_account.pubkey(), mint, owner)
                .unwrap();

        // Build and send transaction
        let recent_blockhash = program.rpc().get_latest_blockhash().unwrap();
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[create_account_instr, init_account_instr],
            Some(&payer.pubkey()),
            &[payer, &token_account],
            recent_blockhash,
        );

        program
            .rpc()
            .send_and_confirm_transaction(&transaction)
            .unwrap();

        token_account.pubkey()
    }

    /// Mint tokens to an account
    async fn mint_tokens(
        program: &Program<Arc<Keypair>>,
        authority: &Keypair,
        mint: &Pubkey,
        to: &Pubkey,
        amount: u64,
    ) {
        // Create mint to instruction
        let mint_to_instr =
            token_instruction::mint_to(&token::ID, mint, to, &authority.pubkey(), &[], amount)
                .unwrap();

        // Build and send transaction
        let recent_blockhash = program.rpc().get_latest_blockhash().unwrap();
        let transaction = solana_sdk::transaction::Transaction::new_signed_with_payer(
            &[mint_to_instr],
            Some(&authority.pubkey()),
            &[authority],
            recent_blockhash,
        );

        program
            .rpc()
            .send_and_confirm_transaction(&transaction)
            .unwrap();
    }

    /// Create a pool using Anchor's request builder
    async fn create_pool(
        program: &Program<Arc<Keypair>>,
        admin: &Keypair,
        token_a_mint: Pubkey,
        token_b_mint: Pubkey,
        fee_tier: u16,
    ) -> (Pubkey, Pubkey) {
        // Order token mints to ensure token0 < token1
        let (token0, token1) = if token_a_mint < token_b_mint {
            (token_a_mint, token_b_mint)
        } else {
            (token_b_mint, token_a_mint)
        };

        // Find or create token pair
        let token_pair_seeds = [b"token_pair".as_ref(), token0.as_ref(), token1.as_ref()];
        let (token_pair, _) = Pubkey::find_program_address(&token_pair_seeds, &FLUXA_PROGRAM_ID);

        // Try to create the token pair if it doesn't exist
        let token_pair_account = program.rpc().get_account(&token_pair);
        if token_pair_account.is_err() {
            // Create token pair first
            let create_token_pair = program
                .request()
                .accounts(amm_core::accounts::CreateTokenPair {
                    authority: admin.pubkey(),
                    token_pair,
                    token_a_mint: token0,
                    token_b_mint: token1,
                    system_program: system_program::ID,
                    rent: sysvar::rent::ID,
                })
                .args(amm_core::instruction::CreateTokenPair {});

            create_token_pair.signer(admin).send().unwrap();
        }

        // Calculate PDA for pool (deterministic)
        let seeds = [
            b"pool".as_ref(),
            &token0.to_bytes(),
            &token1.to_bytes(),
            &fee_tier.to_le_bytes(),
        ];
        let (pool_pubkey, _) = Pubkey::find_program_address(&seeds, &FLUXA_PROGRAM_ID);

        // Calculate pool authority (PDA)
        let authority_seeds = [b"authority".as_ref(), pool_pubkey.as_ref()];
        let (pool_authority, _) = Pubkey::find_program_address(&authority_seeds, &FLUXA_PROGRAM_ID);

        // Create token accounts for the pool
        let pool_token0_account =
            Self::create_token_account(program, admin, &token0, &pool_authority).await;

        let pool_token1_account =
            Self::create_token_account(program, admin, &token1, &pool_authority).await;

        // Build the request with accounts
        let request = program
            .request()
            .accounts(amm_core::accounts::InitializePool {
                payer: admin.pubkey(),
                pool: pool_pubkey,
                token_pair,
                token_a_mint: token0,
                token_b_mint: token1,
                token_a_vault: pool_token0_account,
                token_b_vault: pool_token1_account,
                token_program: token::ID,
                system_program: system_program::ID,
                rent: sysvar::rent::ID,
            })
            .args(amm_core::instruction::InitializePool {
                initial_sqrt_price: 1_000_000_000_000, // 1.0
                fee_tier,
            });

        // Add the admin as a signer and send the transaction
        request.signer(admin).send().unwrap();

        (pool_pubkey, pool_authority)
    }

    /// Create a position in the pool using Anchor's request builder
    async fn create_position(
        &self,
        user: &Keypair,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
    ) -> Pubkey {
        // Calculate sqrt price bounds from ticks
        let sqrt_price_lower = tick_to_sqrt_price(tick_lower).unwrap();
        let sqrt_price_upper = tick_to_sqrt_price(tick_upper).unwrap();

        // Calculate token amounts needed for the position
        let pool_state = self.get_pool_state().await;
        let current_sqrt_price = pool_state.sqrt_price;

        // Determine which token amounts are needed based on current price
        let (amount0, amount1) = if current_sqrt_price <= sqrt_price_lower {
            // Current price is below the position range, only token0 needed
            let amount0 = get_amount0_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);
            (amount0, 0)
        } else if current_sqrt_price >= sqrt_price_upper {
            // Current price is above the position range, only token1 needed
            let amount1 = get_amount1_delta(sqrt_price_lower, sqrt_price_upper, liquidity, true);
            (0, amount1)
        } else {
            // Current price is within the position range, both tokens needed
            let amount0 = get_amount0_delta(current_sqrt_price, sqrt_price_upper, liquidity, true);
            let amount1 = get_amount1_delta(sqrt_price_lower, current_sqrt_price, liquidity, true);
            (amount0, amount1)
        };

        // Get token accounts based on actual mint order in the pool
        let token_pair = self.get_token_pair(&pool_state).await;
        let (token0_account, token1_account) = if self.token_a_mint == token_pair.token_a_mint {
            (self.token_a_account_a, self.token_b_account_a)
        } else {
            (self.token_b_account_a, self.token_a_account_a)
        };

        // Create NFT mint for position
        let position_mint = Self::create_mint(&self.program, &self.admin).await;

        // Create NFT token account for position
        let position_token_account =
            Self::create_token_account(&self.program, &self.admin, &position_mint, &user.pubkey())
                .await;

        // Mint a single token for the position NFT
        Self::mint_tokens(
            &self.program,
            &self.admin,
            &position_mint,
            &position_token_account,
            1,
        )
        .await;

        // Calculate position PDA
        let position_seeds = [b"position".as_ref(), position_mint.as_ref()];
        let (position_pubkey, _) = Pubkey::find_program_address(&position_seeds, &FLUXA_PROGRAM_ID);

        // Get the pool token vault accounts
        let (token_a_vault, token_b_vault) = self.get_pool_token_vaults().await;

        // Build create position request using Anchor's request builder
        let request = self
            .program
            .request()
            .accounts(amm_core::accounts::CreatePosition {
                owner: user.pubkey(),
                position: position_pubkey,
                pool: self.pool_pubkey,
                token_a_account: token0_account,
                token_b_account: token1_account,
                token_a_vault,
                token_b_vault,
                token_program: token::ID,
                system_program: system_program::ID,
                rent: sysvar::rent::ID,
            })
            .args(amm_core::instruction::CreatePosition {
                lower_tick: tick_lower,
                upper_tick: tick_upper,
                liquidity_amount: liquidity,
            });

        // Send transaction
        request.signer(user).send().unwrap();

        position_pubkey
    }

    /// Execute a swap using Anchor's request builder
    async fn swap(
        &self,
        user: &Keypair,
        zero_for_one: bool,
        amount_in: u64,
        sqrt_price_limit: u128,
    ) -> (u64, u64, u128) {
        // Get current pool state
        let pool_state = self.get_pool_state().await;

        // Get associated token pair
        let token_pair = self.get_token_pair(&pool_state).await;

        // Determine token accounts based on swap direction
        let (token_in_pubkey, token_out_pubkey) = if zero_for_one {
            // Token0 -> Token1
            if self.token_a_mint == token_pair.token_a_mint {
                (self.token_a_account_a, self.token_b_account_a)
            } else {
                (self.token_b_account_a, self.token_a_account_a)
            }
        } else {
            // Token1 -> Token0
            if self.token_a_mint == token_pair.token_a_mint {
                (self.token_b_account_a, self.token_a_account_a)
            } else {
                (self.token_a_account_a, self.token_b_account_a)
            }
        };

        // Use actual sqrt_price_limit or default to min/max based on direction
        let actual_sqrt_price_limit = if sqrt_price_limit == 0 {
            if zero_for_one {
                // For zero_for_one, use minimum sqrt price (lowest possible)
                tick_to_sqrt_price(-887272).unwrap()
            } else {
                // For one_for_zero, use maximum sqrt price (highest possible)
                tick_to_sqrt_price(887272).unwrap()
            }
        } else {
            sqrt_price_limit
        };

        // Record token balances before swap
        let token_in_before = self.get_token_balance(&token_in_pubkey).await;
        let token_out_before = self.get_token_balance(&token_out_pubkey).await;

        // Build swap request using Anchor's request builder
        let request = self
            .program
            .request()
            .accounts(amm_core::accounts::Swap {
                payer: user.pubkey(),
                pool: self.pool_pubkey,
                token_in: token_in_pubkey,
                token_out: token_out_pubkey,
                pool_authority: self.pool_authority,
                token_program: token::ID,
            })
            .args(amm_core::instruction::Swap {
                amount_in,
                minimum_amount_out: 0, // No slippage protection for tests
                sqrt_price_limit: actual_sqrt_price_limit,
                zero_for_one,
            });

        // Send transaction
        request.signer(user).send().await.unwrap();

        // Get token balances after swap
        let token_in_after = self.get_token_balance(&token_in_pubkey).await;
        let token_out_after = self.get_token_balance(&token_out_pubkey).await;

        // Calculate actual amounts
        let amount_in_used = token_in_before - token_in_after;
        let amount_out = token_out_after - token_out_before;

        // Get updated pool state to get new sqrt_price
        let updated_pool_state = self.get_pool_state().await;
        let next_sqrt_price = updated_pool_state.sqrt_price;

        (amount_in_used, amount_out, next_sqrt_price)
    }

    /// Get token account balance
    async fn get_token_balance(&self, token_account: &Pubkey) -> u64 {
        let account = self.program.rpc().get_account(token_account).await.unwrap();
        let token_account = TokenAccount::unpack(&account.data).unwrap();
        token_account.amount
    }

    /// Get current pool state
    async fn get_pool_state(&self) -> Pool {
        // Fetch pool account data
        let account = self.program.rpc().get_account(&self.pool_pubkey).unwrap();

        // Deserialize the pool data using Anchor's account deserialization
        // Skip the 8-byte discriminator
        let mut data = &account.data[8..];
        Pool::deserialize(&mut data).unwrap()
    }

    /// Get associated token pair for a pool
    async fn get_token_pair(&self, pool: &Pool) -> TokenPair {
        // Find token pair address using the same seeds used in create_pool
        let (token0, token1) = if pool.token_a_mint < pool.token_b_mint {
            (pool.token_a_mint, pool.token_b_mint)
        } else {
            (pool.token_b_mint, pool.token_a_mint)
        };

        let token_pair_seeds = [b"token_pair".as_ref(), token0.as_ref(), token1.as_ref()];
        let (token_pair_pubkey, _) =
            Pubkey::find_program_address(&token_pair_seeds, &FLUXA_PROGRAM_ID);

        // Fetch token pair account data
        let account = self.program.rpc().get_account(&token_pair_pubkey).unwrap();

        // Deserialize the token pair data
        let mut data = &account.data[8..];
        TokenPair::deserialize(&mut data).unwrap()
    }

    /// Get token vault accounts for a pool
    async fn get_pool_token_vaults(&self) -> (Pubkey, Pubkey) {
        // Get the pool state to access token mint information
        let pool_state = self.get_pool_state().await;

        // Get the token pair info to understand token ordering
        let token_pair = self.get_token_pair(&pool_state).await;

        // Now query the token accounts owned by the pool authority
        let accounts = self
            .program
            .rpc()
            .get_token_accounts_by_owner(
                &self.pool_authority,
                spl_token::rpc::TokenAccountsFilter::Mint(pool_state.token_a_mint),
            )
            .unwrap();

        let token_a_vault = if !accounts.is_empty() {
            Pubkey::from_str(&accounts[0].pubkey).unwrap()
        } else {
            panic!("Token A vault not found for pool");
        };

        let accounts = self
            .program
            .rpc()
            .get_token_accounts_by_owner(
                &self.pool_authority,
                spl_token::rpc::TokenAccountsFilter::Mint(pool_state.token_b_mint),
            )
            .unwrap();

        let token_b_vault = if !accounts.is_empty() {
            Pubkey::from_str(&accounts[0].pubkey).unwrap()
        } else {
            panic!("Token B vault not found for pool");
        };

        (token_a_vault, token_b_vault)
    }

    /// Get token A vault for a pool
    async fn get_pool_token_a_vault(&self) -> Pubkey {
        self.get_pool_token_vaults().await.0
    }

    /// Get token B vault for a pool
    async fn get_pool_token_b_vault(&self) -> Pubkey {
        self.get_pool_token_vaults().await.1
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
        let tick_index = sqrt_price_to_tick(pool_state.sqrt_price).unwrap();
        assert_eq!(tick_index, pool_state.tick_current);

        // Verify that converting back to sqrt price gives a close value
        let sqrt_price_from_tick = tick_to_sqrt_price(pool_state.tick_current).unwrap();
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
        let fixture = TestFixture::new().await;

        // Define position parameters
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        // Create position
        let position_pubkey = fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

        // Calculate expected token amounts
        let sqrt_price_lower = tick_to_sqrt_price(tick_lower).unwrap();
        let sqrt_price_upper = tick_to_sqrt_price(tick_upper).unwrap();

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
        let fixture = TestFixture::new().await;

        // First create a position to provide liquidity
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

        // Get initial pool state
        let initial_pool_state = fixture.get_pool_state().await;
        let initial_sqrt_price = initial_pool_state.sqrt_price;
        let initial_liquidity = initial_pool_state.liquidity;

        // Execute a swap: token0 for token1
        let amount_in = 1_000_000u64; // 1 token0 (with 6 decimals)
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

        // Allow for minor difference due to possible tick crossing in actual implementation
        let diff = if next_sqrt_price > expected_next_sqrt_price {
            next_sqrt_price - expected_next_sqrt_price
        } else {
            expected_next_sqrt_price - next_sqrt_price
        };

        assert!(diff <= initial_sqrt_price / 1_000); // 0.1% error tolerance

        // Verify amount calculations
        let expected_amount0_used =
            get_amount0_delta(next_sqrt_price, initial_sqrt_price, initial_liquidity, true);

        let expected_amount1_out = get_amount1_delta(
            next_sqrt_price,
            initial_sqrt_price,
            initial_liquidity,
            false,
        );

        // Allow for minor differences due to possible tick crossing or fees
        let amount_in_diff = (amount_in_used as i128 - expected_amount0_used as i128).abs() as u64;
        let amount_out_diff = (amount_out as i128 - expected_amount1_out as i128).abs() as u64;

        assert!(amount_in_diff <= amount_in_used / 100); // 1% error tolerance
        assert!(amount_out_diff <= amount_out / 100); // 1% error tolerance

        // Verify price movement direction
        assert!(next_sqrt_price < initial_sqrt_price); // Price should decrease when selling token0
    }

    /// Test fee calculation during swaps
    #[tokio::test]
    async fn test_fee_calculation_during_swap() {
        let fixture = TestFixture::new().await;

        // First create a position to provide liquidity
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

        // Get pool state after position creation
        let pool_state = fixture.get_pool_state().await;

        // Execute a swap
        let amount_in = 1_000_000u64; // 1 token
        let zero_for_one = true;

        // Record token balances before swap to calculate actual token movements
        let token_pair = fixture.get_token_pair(&pool_state).await;
        let (token_in_pubkey, _) = if zero_for_one {
            // Token0 -> Token1
            if fixture.token_a_mint == token_pair.token_a_mint {
                (fixture.token_a_account_a, fixture.token_b_account_a)
            } else {
                (fixture.token_b_account_a, fixture.token_a_account_a)
            }
        } else {
            // Token1 -> Token0
            if fixture.token_a_mint == token_pair.token_a_mint {
                (fixture.token_b_account_a, fixture.token_a_account_a)
            } else {
                (fixture.token_a_account_a, fixture.token_b_account_a)
            }
        };

        let token_in_before = fixture.get_token_balance(&token_in_pubkey).await;

        let (amount_in_used, _, _) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Get token balances after swap
        let token_in_after = fixture.get_token_balance(&token_in_pubkey).await;
        let actual_amount_used = token_in_before - token_in_after;

        // Calculate expected fee
        let expected_fee = calculate_fee(amount_in_used, pool_state.fee_tier);

        // Verify that the fee calculation is correct
        assert!(expected_fee > 0);
        assert_eq!(
            expected_fee,
            amount_in_used * (pool_state.fee_tier as u64) / 1_000_000
        );

        // Verify that the actual amount used matches what our calculation predicted
        assert_eq!(actual_amount_used, amount_in_used);

        // Get updated pool state to check fee accumulation
        let updated_pool_state = fixture.get_pool_state().await;

        // Verify fee accumulation in the pool (should be tracked in fee_growth_global fields)
        if zero_for_one {
            // When swapping token0 for token1, fee0 should increase
            assert!(updated_pool_state.fee_growth_global_0 > pool_state.fee_growth_global_0);
        } else {
            // When swapping token1 for token0, fee1 should increase
            assert!(updated_pool_state.fee_growth_global_1 > pool_state.fee_growth_global_1);
        }
    }

    /// Test cross-tick swap calculations
    #[tokio::test]
    async fn test_cross_tick_swap_calculations() {
        let fixture = TestFixture::new().await;

        // Create positions that span multiple tick ranges
        // Position 1: Current price range
        fixture
            .create_position(&fixture.user_a, -100, 100, 10_000_000_000u128)
            .await;

        // Position 2: Higher price range
        fixture
            .create_position(&fixture.user_a, 100, 300, 5_000_000_000u128)
            .await;

        // Get initial pool state
        let initial_pool_state = fixture.get_pool_state().await;
        let initial_sqrt_price = initial_pool_state.sqrt_price;
        let initial_tick = initial_pool_state.tick_current;

        // Execute a large swap that should cross the tick boundary
        // Swapping token1 for token0 (one_for_zero) to increase price
        let zero_for_one = false;
        let amount_in = 100_000_000u64; // Large amount to ensure tick crossing

        let (amount_in_used, amount_out, next_sqrt_price) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Get updated pool state after swap
        let updated_pool_state = fixture.get_pool_state().await;

        // Verify that we've crossed the tick boundary at tick 100
        assert!(initial_tick < 100);
        assert!(updated_pool_state.tick_current >= 100);

        // Verify price movement
        assert!(next_sqrt_price > initial_sqrt_price);

        // When crossing a tick, the liquidity should change
        // The total available liquidity should decrease as we move out of the first position's range
        // and into the second position's range
        assert_ne!(updated_pool_state.liquidity, initial_pool_state.liquidity);

        // For the return journey, execute a swap in the opposite direction
        let (reverse_amount_in_used, reverse_amount_out, final_sqrt_price) = fixture
            .swap(
                &fixture.user_b,
                !zero_for_one, // Opposite direction
                amount_out,    // Use the output from previous swap
                0,             // No price limit
            )
            .await;

        // Get final pool state
        let final_pool_state = fixture.get_pool_state().await;

        // Verify we crossed back through the tick boundary
        assert!(final_pool_state.tick_current < 100);

        // Price should have returned close to the initial price
        // (allowing for fees and rounding)
        let price_ratio = if final_sqrt_price > initial_sqrt_price {
            final_sqrt_price as f64 / initial_sqrt_price as f64
        } else {
            initial_sqrt_price as f64 / final_sqrt_price as f64
        };

        // The price should be within 5% of the original price after round trip
        assert!(price_ratio < 1.05);

        // The liquidity should approximately match the initial liquidity after
        // returning to the original price range
        let liquidity_diff = if final_pool_state.liquidity > initial_pool_state.liquidity {
            final_pool_state.liquidity - initial_pool_state.liquidity
        } else {
            initial_pool_state.liquidity - final_pool_state.liquidity
        };

        // Liquidity should be within 1% of original after returning to the same range
        assert!(liquidity_diff <= initial_pool_state.liquidity / 100);
    }

    /// Test liquidity addition and removal across price ranges
    #[tokio::test]
    async fn test_liquidity_management_across_price_ranges() {
        let fixture = TestFixture::new().await;

        // Get initial pool state
        let initial_pool_state = fixture.get_pool_state().await;
        let initial_liquidity = initial_pool_state.liquidity;

        // Create multiple positions across different price ranges
        let positions = vec![
            (-100, 100, 10_000_000_000u128), // Position around current price
            (-500, -200, 5_000_000_000u128), // Position below current price
            (200, 500, 5_000_000_000u128),   // Position above current price
        ];

        // Create all positions
        for (i, (tick_lower, tick_upper, liquidity)) in positions.iter().enumerate() {
            let position_pubkey = fixture
                .create_position(&fixture.user_a, *tick_lower, *tick_upper, *liquidity)
                .await;

            // Check pool state after each position creation
            let pool_state_after = fixture.get_pool_state().await;

            // Only the position that includes the current price should affect active liquidity
            if i == 0 {
                // First position is in range, so liquidity should increase
                assert!(pool_state_after.liquidity > initial_liquidity);
                assert_eq!(pool_state_after.liquidity, initial_liquidity + *liquidity);
            } else {
                // Other positions are out of range, so liquidity should not change
                assert_eq!(
                    pool_state_after.liquidity,
                    initial_liquidity + positions[0].2
                );
            }
        }

        // Execute a swap to move price into the range of the third position
        // (Swapping token1 for token0 to increase price)
        let zero_for_one = false;
        let amount_in = 50_000_000u64; // Large enough to cross into the higher range

        let (_, _, _) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Get pool state after the price increase
        let pool_state_after_up = fixture.get_pool_state().await;

        // Verify that the price is now in the third position's range
        assert!(pool_state_after_up.tick_current >= positions[2].0);
        assert!(pool_state_after_up.tick_current <= positions[2].1);

        // Verify that the active liquidity now includes the third position
        // but not the first position (which is now out of range)
        assert_eq!(pool_state_after_up.liquidity, positions[2].2);

        // Now execute a swap to decrease price to the first position's range
        let (_, _, _) = fixture
            .swap(
                &fixture.user_a,
                !zero_for_one, // Opposite direction
                amount_in * 2, // Enough to get back to the first position
                0,             // No price limit
            )
            .await;

        // Get pool state after the price decrease
        let pool_state_after_down = fixture.get_pool_state().await;

        // Verify that the price is back in the first position's range
        assert!(pool_state_after_down.tick_current >= positions[0].0);
        assert!(pool_state_after_down.tick_current <= positions[0].1);

        // Verify that the active liquidity now includes the first position
        assert_eq!(pool_state_after_down.liquidity, positions[0].2);

        // Execute a swap to move price into the second position's range (lower)
        let (_, _, _) = fixture
            .swap(
                &fixture.user_a,
                !zero_for_one, // Continue decreasing price
                amount_in * 3, // Large enough to move to the lower range
                0,             // No price limit
            )
            .await;

        // Get pool state after the further price decrease
        let pool_state_after_lowest = fixture.get_pool_state().await;

        // Verify that the price is now in the second position's range
        assert!(pool_state_after_lowest.tick_current >= positions[1].0);
        assert!(pool_state_after_lowest.tick_current <= positions[1].1);

        // Verify that the active liquidity now includes the second position
        assert_eq!(pool_state_after_lowest.liquidity, positions[1].2);
    }

    /// Test mathematical consistency across swap path
    #[tokio::test]
    async fn test_mathematical_consistency_in_swap_path() {
        let fixture = TestFixture::new().await;

        // Create a position to provide liquidity
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

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

        // Calculate the expected amount out after fees and slippage
        // This is a simplified calculation
        let expected_amount0_after_fees = amount0_in - expected_fee0;

        // The difference between what we put in and what we got back
        // should be approximately equal to the fees paid
        let diff = amount0_in - amount0_out;

        // Allow for some rounding error and slippage
        let error_margin = 10 + amount0_in / 1000; // Base error + 0.1% of amount
        assert!(
            (diff as i64 - (expected_fee0 + expected_fee1) as i64).abs() <= error_margin as i64
        );

        // Verify price movement is symmetric (adjusted for fees)
        // The price should return close to the original price
        let price_diff = if initial_sqrt_price > next_sqrt_price2 {
            initial_sqrt_price - next_sqrt_price2
        } else {
            next_sqrt_price2 - initial_sqrt_price
        };

        // The price difference should be small relative to the initial price
        assert!(price_diff <= initial_sqrt_price / 1_000);

        // Verify that the pool state is consistent after the round trip
        let final_pool_state = fixture.get_pool_state().await;

        // The liquidity should remain the same
        assert_eq!(final_pool_state.liquidity, initial_liquidity);

        // The fees should have been collected
        assert!(final_pool_state.fee_growth_global_0 > initial_pool_state.fee_growth_global_0);
        assert!(final_pool_state.fee_growth_global_1 > initial_pool_state.fee_growth_global_1);
    }

    /// Test math functions with extreme values
    #[tokio::test]
    async fn test_math_functions_with_extreme_values() {
        // This test doesn't interact with the blockchain, so we can test directly

        // Test with minimum tick
        let min_tick = -887272; // Adjust based on Fluxa's configured min tick
        let min_sqrt_price = tick_to_sqrt_price(min_tick).unwrap();
        let min_tick_roundtrip = sqrt_price_to_tick(min_sqrt_price).unwrap();

        // Allow for at most 1 tick of rounding error
        assert!((min_tick - min_tick_roundtrip).abs() <= 1);

        // Test with maximum tick
        let max_tick = 887272; // Adjust based on Fluxa's configured max tick
        let max_sqrt_price = tick_to_sqrt_price(max_tick).unwrap();
        let max_tick_roundtrip = sqrt_price_to_tick(max_sqrt_price).unwrap();

        // Allow for at most 1 tick of rounding error
        assert!((max_tick - max_tick_roundtrip).abs() <= 1);

        // Test with very small liquidity
        let small_liquidity = 1u128;
        let sqrt_price_a = tick_to_sqrt_price(0).unwrap();
        let sqrt_price_b = tick_to_sqrt_price(1).unwrap();

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
        if amount0 > 0 {
            let ratio0 =
                (amount0_large as u128) * small_liquidity / (amount0 as u128) / large_liquidity;
            // Ratios should be close to 1, allowing for some rounding error
            assert!(ratio0 >= 9_999 && ratio0 <= 10_001); // 0.01% error margin
        }

        if amount1 > 0 {
            let ratio1 =
                (amount1_large as u128) * small_liquidity / (amount1 as u128) / large_liquidity;
            // Ratios should be close to 1, allowing for some rounding error
            assert!(ratio1 >= 9_999 && ratio1 <= 10_001); // 0.01% error margin
        }
    }

    /// Test event emissions for swaps using Anchor's event parsing
    #[tokio::test]
    async fn test_swap_event_emissions() {
        let fixture = TestFixture::new().await;

        // Create a position to provide liquidity
        let tick_lower = -100;
        let tick_upper = 100;
        let liquidity = 10_000_000_000u128; // 10K liquidity

        fixture
            .create_position(&fixture.user_a, tick_lower, tick_upper, liquidity)
            .await;

        // Set up event parser
        let mut event_parser = EventParser::new(fixture.program.id());
        // Register the event we want to listen for
        event_parser.add_event::<SwapEvent>("SwapEvent");

        // Create a client with event handling capabilities
        let client_with_events = Client::new_with_options(
            Cluster::Localnet,
            Arc::new(fixture.admin.clone()),
            CommitmentConfig::confirmed(),
        );

        // Execute a swap
        let amount_in = 1_000_000u64;
        let zero_for_one = true;

        // Subscribe to program logs before executing swap
        let subscription_id = client_with_events
            .program(fixture.program.id())
            .event_subscription()
            .filter(format!("program={}", fixture.program.id()))
            .subscribe()
            .unwrap();

        // Execute the swap
        let (amount_in_used, amount_out, next_sqrt_price) = fixture
            .swap(
                &fixture.user_a,
                zero_for_one,
                amount_in,
                0, // No price limit
            )
            .await;

        // Get the swap event
        let mut events = Vec::new();
        for _ in 0..5 {
            // Try for up to 5 seconds to get the event
            if let Ok(mut new_events) = client_with_events
                .program(fixture.program.id())
                .event_subscription()
                .poll_for_events(subscription_id, Some(1))
            {
                events.append(&mut new_events);
                if !events.is_empty() {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        // Unsubscribe
        client_with_events
            .program(fixture.program.id())
            .event_subscription()
            .unsubscribe(subscription_id)
            .unwrap();

        // Assert we got the event
        assert!(!events.is_empty());

        // Parse the events
        let parsed_events: Vec<_> = event_parser.parse(&events).collect();

        // Find our swap event
        let swap_event_opt = parsed_events.iter().find_map(|event| {
            if let EventContext::Unknown { name, .. } = event {
                if name == "SwapEvent" {
                    return Some(event);
                }
            }
            None
        });

        // Assert we found the event
        assert!(swap_event_opt.is_some());

        // In a real implementation, we would deserialize the event data and verify
        // that it matches our expectations (amounts, price, etc.)
    }
}

/// Struct for Anchor-based event parsing
/// This would match the event defined in the program
#[derive(Debug, Clone, AnchorDeserialize)]
struct SwapEvent {
    amount0: i64,
    amount1: i64,
    sqrt_price_x64: u128,
    liquidity: u128,
    tick: i32,
}
