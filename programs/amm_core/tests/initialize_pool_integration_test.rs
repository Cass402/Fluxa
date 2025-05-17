// /tests/initialize_pool.rs

use anchor_lang::{
    prelude::Pubkey, // AccountDeserialize can be brought in if specifically needed later
    solana_program::{program_pack::Pack, system_instruction},
    AccountDeserialize, // Added for Pool::try_deserialize
    InstructionData,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext}; // Explicit imports, Added BanksClientError
use solana_sdk::{
    instruction::AccountMeta, // Import AccountMeta directly
    instruction::Instruction,
    signature::{Keypair, Signer},
    sysvar, // For sysvar::rent::id()
    transaction::Transaction,
    transport,
};

// Assuming your crate is named amm_core
use amm_core::{
    self,                                                     // Import the crate itself
    errors::ErrorCode,                                        // Import ErrorCode
    instruction::InitializePoolHandler as InitializePoolData, // Correct instruction data struct
    state::pool::Pool,
    ID as PROGRAM_ID, // Use the declared program ID
};

// Helper function to create a mint
async fn create_mint(
    context: &mut ProgramTestContext,
    authority: &Pubkey,
) -> transport::Result<(Keypair, Pubkey)> {
    let mint_keypair = Keypair::new();
    let rent = context.banks_client.get_rent().await.unwrap();
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);

    let transaction = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &context.payer.pubkey(),
                &mint_keypair.pubkey(),
                mint_rent,
                spl_token::state::Mint::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint(
                &spl_token::id(),
                &mint_keypair.pubkey(),
                authority,
                None,
                0,
            )
            .unwrap(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer, &mint_keypair.insecure_clone()], // Clone mint_keypair for signing
        context.last_blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await?;
    let pubkey = mint_keypair.pubkey();
    Ok((mint_keypair, pubkey))
}

// Helper function to create a token account
#[allow(dead_code)]
async fn create_token_account(
    context: &mut ProgramTestContext,
    mint_pubkey: &Pubkey,
    owner: &Pubkey,
) -> transport::Result<Pubkey> {
    let token_account_keypair = Keypair::new();
    let rent = context.banks_client.get_rent().await.unwrap();
    let token_account_rent = rent.minimum_balance(spl_token::state::Account::LEN);

    let transaction = Transaction::new_signed_with_payer(
        &[
            system_instruction::create_account(
                &context.payer.pubkey(),
                &token_account_keypair.pubkey(),
                token_account_rent,
                spl_token::state::Account::LEN as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_account(
                &spl_token::id(),
                &token_account_keypair.pubkey(),
                mint_pubkey,
                owner,
            )
            .unwrap(),
        ],
        Some(&context.payer.pubkey()),
        &[&context.payer, &token_account_keypair],
        context.last_blockhash,
    );
    context
        .banks_client
        .process_transaction(transaction)
        .await?;
    Ok(token_account_keypair.pubkey())
}

#[tokio::test]
async fn test_initialize_pool_success() {
    let program_test = ProgramTest::new(
        "amm_core", // Replace with your program name if different
        PROGRAM_ID,
        // Try with the expected path first. If "private" or signature errors persist,
        // this might indicate a deeper issue with Anchor version / solana-program-test compatibility.
        // The compiler's suggestion `crate::amm_core::entry` is usually for within the lib crate.
        None,
    );

    // Add System program manually if not implicitly added
    // program_test.add_program("system_program", solana_program::system_program::id(), None);
    // program_test.add_program("token_program", spl_token::id(), None);

    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone(); // Payer for transactions
    let factory_keypair = Keypair::new(); // For the factory account

    // 1. Create Mints (ensure canonical order for PDA derivation)
    // We'll create two mints and then sort them by pubkey to ensure canonical order.
    let (mut mint_a_keypair, mut mint_a_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();
    let (mut mint_b_keypair, mut mint_b_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();

    // Ensure canonical order (mint_a.key < mint_b.key)
    if mint_a_pubkey > mint_b_pubkey {
        std::mem::swap(&mut mint_a_keypair, &mut mint_b_keypair);
        std::mem::swap(&mut mint_a_pubkey, &mut mint_b_pubkey);
    }

    println!("Mint A: {mint_a_pubkey}");
    println!("Mint B: {mint_b_pubkey}");

    // 2. Define PDAs for Pool and Vaults
    let (pool_pda, pool_bump) = Pubkey::find_program_address(
        &[
            b"pool".as_ref(),
            mint_a_pubkey.as_ref(),
            mint_b_pubkey.as_ref(),
        ],
        &PROGRAM_ID,
    );

    // Vault PDAs are derived with the pool PDA as authority (as per constraints)
    // but for init, the authority is the pool PDA itself.
    // The seeds for the token vaults are not explicitly defined by the PDA derivation
    // in InitializePool struct in lib.rs but by token::authority = pool.
    // For the test, we just need keypairs for them as they are initialized by the program.
    let pool_vault_a_keypair = Keypair::new();
    let pool_vault_b_keypair = Keypair::new();

    // 3. Define Instruction Parameters
    let initial_sqrt_price_q64: u128 = 79228162514264337593543950336; // Example: 1 * 2^64 (for price 1)
    let fee_rate: u16 = 30; // 0.3%
    let tick_spacing: u16 = 60; // Example tick spacing

    // 4. Construct the instruction
    // For solana-program-test, construct AccountMeta vector manually.
    let account_metas = vec![
        AccountMeta::new(pool_pda, false), // pool (writable, not signer by instruction itself for init)
        AccountMeta::new_readonly(mint_a_pubkey, false), // mint_a
        AccountMeta::new_readonly(mint_b_pubkey, false), // mint_b
        AccountMeta::new_readonly(factory_keypair.pubkey(), false), // factory
        AccountMeta::new(pool_vault_a_keypair.pubkey(), true), // pool_vault_a (writable, signer)
        AccountMeta::new(pool_vault_b_keypair.pubkey(), true), // pool_vault_b (writable, signer)
        AccountMeta::new(payer.pubkey(), true), // payer (writable, signer)
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false), // system_program
        AccountMeta::new_readonly(spl_token::ID, false), // token_program
        AccountMeta::new_readonly(sysvar::rent::ID, false), // rent
    ];

    let instruction_data_struct = InitializePoolData {
        initial_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: account_metas,
        data: instruction_data_struct.data(),
    };

    // Signers: payer, and also vault keypairs because they are being initialized.
    // The `InitializePool` struct in `lib.rs` uses `init` for `pool_vault_a` and `pool_vault_b`,
    // meaning these accounts must be signers if they are new.
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &pool_vault_a_keypair, &pool_vault_b_keypair], // Vaults must sign as they are initialized.
        context.last_blockhash,
    );

    context
        .banks_client
        .process_transaction(transaction)
        .await
        .unwrap();

    // 5. State Verification
    // Fetch and verify Pool account
    let pool_account_data = context
        .banks_client
        .get_account(pool_pda)
        .await
        .expect("Pool account not found")
        .expect("Pool account is empty");

    let pool_state = Pool::try_deserialize(&mut pool_account_data.data.as_slice()).unwrap();

    assert_eq!(pool_state.bump, pool_bump);
    assert_eq!(pool_state.factory, factory_keypair.pubkey());
    assert_eq!(pool_state.token0_mint, mint_a_pubkey); // mint_a is token0 due to canonical order
    assert_eq!(pool_state.token1_mint, mint_b_pubkey); // mint_b is token1
    assert_eq!(pool_state.token0_vault, pool_vault_a_keypair.pubkey());
    assert_eq!(pool_state.token1_vault, pool_vault_b_keypair.pubkey());
    assert_eq!(pool_state.fee_rate, fee_rate);
    assert_eq!(pool_state.tick_spacing, tick_spacing);
    assert_eq!(pool_state.sqrt_price_q64, initial_sqrt_price_q64);

    // Calculate expected_current_tick (this requires access to math::sqrt_price_q64_to_tick)
    // For now, we'll assert it's not the default i32 (0), assuming successful calculation.
    // You might need to replicate or expose the math function for a precise check.
    let expected_current_tick =
        amm_core::math::sqrt_price_q64_to_tick(initial_sqrt_price_q64).unwrap();
    assert_eq!(pool_state.current_tick, expected_current_tick);

    assert_eq!(pool_state.liquidity, 0); // Initial liquidity is 0
    assert!(!pool_state.tick_bitmap_data.is_empty()); // Should be initialized (empty BTreeMap serialized)

    // Fetch and verify Vault A
    let vault_a_account_data = context
        .banks_client
        .get_account(pool_vault_a_keypair.pubkey())
        .await
        .expect("Vault A not found")
        .expect("Vault A is empty");
    let vault_a_state = spl_token::state::Account::unpack(&vault_a_account_data.data).unwrap();
    assert_eq!(vault_a_state.mint, mint_a_pubkey);
    assert_eq!(vault_a_state.owner, pool_pda); // Authority should be the pool PDA

    // Fetch and verify Vault B
    let vault_b_account_data = context
        .banks_client
        .get_account(pool_vault_b_keypair.pubkey())
        .await
        .expect("Vault B not found")
        .expect("Vault B is empty");
    let vault_b_state = spl_token::state::Account::unpack(&vault_b_account_data.data).unwrap();
    assert_eq!(vault_b_state.mint, mint_b_pubkey);
    assert_eq!(vault_b_state.owner, pool_pda); // Authority should be the pool PDA

    println!("Successfully initialized pool and verified state!");
}

#[tokio::test]
async fn test_initialize_pool_mints_not_canonical() {
    let program_test = ProgramTest::new("amm_core", PROGRAM_ID, None);
    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();
    let factory_keypair = Keypair::new();

    let (mut mint_a_keypair, mut mint_a_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();
    let (mut mint_b_keypair, mut mint_b_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();

    // Force non-canonical order
    if mint_a_pubkey < mint_b_pubkey {
        std::mem::swap(&mut mint_a_keypair, &mut mint_b_keypair);
        std::mem::swap(&mut mint_a_pubkey, &mut mint_b_pubkey);
    }
    println!("Mint A (non-canonical): {mint_a_pubkey}");
    println!("Mint B (non-canonical): {mint_b_pubkey}");

    let (_pool_pda, _pool_bump) = Pubkey::find_program_address(
        &[
            b"pool".as_ref(),
            mint_a_pubkey.as_ref(), // These will be in non-canonical order for PDA seed
            mint_b_pubkey.as_ref(),
        ],
        &PROGRAM_ID,
    );
    // The actual PDA derivation for the transaction will use these non-canonical mints.
    // However, the instruction handler has a specific check:
    // `if ctx.accounts.mint_a.key() >= ctx.accounts.mint_b.key()`
    // So, the `pool_pda` used in `InitializePoolAccounts` must match what the program expects based on input.
    // For this test, we want to trigger the internal check, so the seeds for `pool_pda` itself don't really matter
    // as much as the direct check on `ctx.accounts.mint_a.key()` and `ctx.accounts.mint_b.key()`.

    // Let's use the non-canonical order for finding the pool PDA that the instruction would attempt to init
    // Note: the seeds in `InitializePool` struct in `lib.rs` *mandate* canonical order for the actual pool PDA.
    // This test checks the explicit validation `ctx.accounts.mint_a.key() >= ctx.accounts.mint_b.key()`.
    // So, the `pool_pda` here should be derived using the *passed* mint_a and mint_b for consistency,
    // even though the program expects them to be canonical for the *actual* stored PDA.
    // The critical part is that `mint_a_pubkey` passed to the instruction is greater than `mint_b_pubkey`.
    let (pool_pda_attempt, _pool_bump_attempt) = Pubkey::find_program_address(
        &[
            b"pool".as_ref(),
            mint_a_pubkey.as_ref(), // Intentionally using the larger key first for seeds to match instruction
            mint_b_pubkey.as_ref(), // Intentionally using the smaller key second
        ],
        &PROGRAM_ID,
    );

    let pool_vault_a_keypair = Keypair::new();
    let pool_vault_b_keypair = Keypair::new();
    let initial_sqrt_price_q64: u128 = 79228162514264337593543950336;
    let fee_rate: u16 = 30;
    let tick_spacing: u16 = 60;

    let account_metas = vec![
        AccountMeta::new(pool_pda_attempt, false),
        AccountMeta::new_readonly(mint_a_pubkey, false), // mint_a (non-canonical larger)
        AccountMeta::new_readonly(mint_b_pubkey, false), // mint_b (non-canonical smaller)
        AccountMeta::new_readonly(factory_keypair.pubkey(), false),
        AccountMeta::new(pool_vault_a_keypair.pubkey(), true),
        AccountMeta::new(pool_vault_b_keypair.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];

    let instruction_data_struct = InitializePoolData {
        initial_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: account_metas,
        data: instruction_data_struct.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &pool_vault_a_keypair, &pool_vault_b_keypair],
        context.last_blockhash,
    );

    let result = context.banks_client.process_transaction(transaction).await;
    assert!(result.is_err());
    let err = result.unwrap_err();

    // Check for the specific custom error
    // This requires that your program maps InstructionError::Custom(0) to ErrorCode::MintsNotInCanonicalOrder.
    // The exact error code (e.g., 6000 for the first custom error) depends on your ErrorCode enum.
    // Assuming ErrorCode::MintsNotInCanonicalOrder is the first error (index 0).
    // You should verify the actual error code mapping.
    // Example: if MintsNotInCanonicalOrder is #[error("Mints must be in canonical order")] `#[msg("Mints must be in canonical order")] MintsNotInCanonicalOrder, // 0x1770 (6000)`
    // Then you'd check for `solana_program::program_error::ProgramError::Custom(ErrorCode::MintsNotInCanonicalOrder as u32)`
    match err {
        BanksClientError::TransactionError(tx_err) => match tx_err {
            solana_sdk::transaction::TransactionError::InstructionError(_, instruction_error) => {
                match instruction_error {
                    solana_sdk::instruction::InstructionError::Custom(code) => {
                        assert_eq!(code, ErrorCode::MintsNotInCanonicalOrder as u32);
                    }
                    _ => panic!("Expected Custom error, got {instruction_error:?}"),
                }
            }
            _ => panic!("Expected InstructionError, got {tx_err:?}"),
        },
        _ => panic!("Expected TransactionError, got {err:?}"),
    }
    println!("Successfully tested non-canonical mint order failure.");
}

#[tokio::test]
async fn test_initialize_pool_invalid_tick_spacing() {
    let program_test = ProgramTest::new("amm_core", PROGRAM_ID, None);
    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();
    let factory_keypair = Keypair::new();

    let (mut mint_a_keypair, mut mint_a_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();
    let (mut mint_b_keypair, mut mint_b_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();

    if mint_a_pubkey > mint_b_pubkey {
        std::mem::swap(&mut mint_a_keypair, &mut mint_b_keypair);
        std::mem::swap(&mut mint_a_pubkey, &mut mint_b_pubkey);
    }

    let (pool_pda, _pool_bump) = Pubkey::find_program_address(
        &[
            b"pool".as_ref(),
            mint_a_pubkey.as_ref(),
            mint_b_pubkey.as_ref(),
        ],
        &PROGRAM_ID,
    );
    let pool_vault_a_keypair = Keypair::new();
    let pool_vault_b_keypair = Keypair::new();
    let initial_sqrt_price_q64: u128 = 79228162514264337593543950336;
    let fee_rate: u16 = 30;
    let tick_spacing: u16 = 0; // Invalid tick spacing

    let account_metas = vec![
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(mint_a_pubkey, false),
        AccountMeta::new_readonly(mint_b_pubkey, false),
        AccountMeta::new_readonly(factory_keypair.pubkey(), false),
        AccountMeta::new(pool_vault_a_keypair.pubkey(), true),
        AccountMeta::new(pool_vault_b_keypair.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];

    let instruction_data_struct = InitializePoolData {
        initial_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: account_metas,
        data: instruction_data_struct.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &pool_vault_a_keypair, &pool_vault_b_keypair],
        context.last_blockhash,
    );
    let result = context.banks_client.process_transaction(transaction).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        // This is BanksClientError
        BanksClientError::TransactionError(
            solana_sdk::transaction::TransactionError::InstructionError(
                _,
                solana_sdk::instruction::InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(code, ErrorCode::InvalidTickSpacing as u32);
        }
        err => panic!("Expected Custom error for InvalidTickSpacing, got {err:?}"),
    }
    println!("Successfully tested invalid tick spacing failure.");
}

#[tokio::test]
async fn test_initialize_pool_invalid_initial_price() {
    let program_test = ProgramTest::new("amm_core", PROGRAM_ID, None);
    let mut context = program_test.start_with_context().await;
    let payer = context.payer.insecure_clone();
    let factory_keypair = Keypair::new();

    let (mut mint_a_keypair, mut mint_a_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();
    let (mut mint_b_keypair, mut mint_b_pubkey) =
        create_mint(&mut context, &payer.pubkey()).await.unwrap();

    if mint_a_pubkey > mint_b_pubkey {
        std::mem::swap(&mut mint_a_keypair, &mut mint_b_keypair);
        std::mem::swap(&mut mint_a_pubkey, &mut mint_b_pubkey);
    }

    let (pool_pda, _pool_bump) = Pubkey::find_program_address(
        &[
            b"pool".as_ref(),
            mint_a_pubkey.as_ref(),
            mint_b_pubkey.as_ref(),
        ],
        &PROGRAM_ID,
    );
    let pool_vault_a_keypair = Keypair::new();
    let pool_vault_b_keypair = Keypair::new();
    let initial_sqrt_price_q64: u128 = 0; // Invalid initial price
    let fee_rate: u16 = 30;
    let tick_spacing: u16 = 60;

    let account_metas_zero_price = vec![
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(mint_a_pubkey, false),
        AccountMeta::new_readonly(mint_b_pubkey, false),
        AccountMeta::new_readonly(factory_keypair.pubkey(), false),
        AccountMeta::new(pool_vault_a_keypair.pubkey(), true),
        AccountMeta::new(pool_vault_b_keypair.pubkey(), true),
        AccountMeta::new(payer.pubkey(), true),
        AccountMeta::new_readonly(anchor_lang::system_program::ID, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
    ];

    let instruction_data_zero_price = InitializePoolData {
        initial_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };
    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: account_metas_zero_price.clone(), // Use clone if metas are reused or define separately
        data: instruction_data_zero_price.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer, &pool_vault_a_keypair, &pool_vault_b_keypair], // Added missing comma here
        context.last_blockhash,
    );
    let result = context.banks_client.process_transaction(transaction).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        // This is BanksClientError
        BanksClientError::TransactionError(
            solana_sdk::transaction::TransactionError::InstructionError(
                _,
                solana_sdk::instruction::InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(code, ErrorCode::InvalidInitialPrice as u32);
        }
        err => panic!("Expected Custom error for InvalidInitialPrice, got {err:?}"),
    }
    println!("Successfully tested invalid initial price failure (zero).");

    // Test with MAX_SQRT_PRICE + 1 (or some very large number > MAX_SQRT_PRICE from your constants.rs)
    // Assuming MAX_SQRT_PRICE is available, e.g. through amm_core::constants::MAX_SQRT_PRICE
    // If not, you might need to define it or use a known large value.
    // For this example, let's use a placeholder for a very large price.
    let too_large_sqrt_price_q64: u128 = u128::MAX / 2; // Example, ensure this is > MAX_SQRT_PRICE

    let instruction_data_large_price = InitializePoolData {
        initial_sqrt_price_q64: too_large_sqrt_price_q64,
        fee_rate,
        tick_spacing,
    };
    let instruction_large_price = Instruction {
        program_id: PROGRAM_ID,
        accounts: account_metas_zero_price, // Reusing metas from above for simplicity
        data: instruction_data_large_price.data(),
    };
    let transaction_large_price = Transaction::new_signed_with_payer(
        &[instruction_large_price],
        Some(&payer.pubkey()),
        &[&payer, &pool_vault_a_keypair, &pool_vault_b_keypair],
        context.last_blockhash,
    );
    let result_large_price = context
        .banks_client
        .process_transaction(transaction_large_price)
        .await;
    assert!(result_large_price.is_err());
    match result_large_price.unwrap_err() {
        BanksClientError::TransactionError(
            solana_sdk::transaction::TransactionError::InstructionError(
                _,
                solana_sdk::instruction::InstructionError::Custom(code),
            ),
        ) => {
            assert_eq!(code, ErrorCode::InvalidInitialPrice as u32);
        }
        err => panic!("Expected Custom error for InvalidInitialPrice (too large), got {err:?}"),
    }
    println!("Successfully tested invalid initial price failure (too large).");
}
