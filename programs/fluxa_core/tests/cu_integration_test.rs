use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use fluxa_core::*;
use solana_program_test::*;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;

// Test program ID (matches the one in lib.rs)
const PROGRAM_ID: &str = "11111111111111111111111111111112";

#[tokio::test]
async fn test_basic_arithmetic_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));

    // Enable compute unit logging
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Create instruction data
    let instruction_data = fluxa_core::instruction::TestBasicArithmetic {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![], // No accounts needed for these tests
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Basic arithmetic CU test failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_sqrt_variants_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestSqrtVariants {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Sqrt variants CU test failed: {:?}", result);
}

#[tokio::test]
async fn test_tick_conversions_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestTickConversions {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Tick conversions CU test failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_mul_div_variants_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestMulDivVariants {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Mul/div variants CU test failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_liquidity_calculations_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestLiquidityCalculations {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Liquidity calculations CU test failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_batch_operations_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestBatchOperations {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Batch operations CU test failed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_edge_cases_cu() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    let instruction_data = fluxa_core::instruction::TestEdgeCases {}.data();

    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Edge cases CU test failed: {:?}", result);
}

// Comprehensive test that runs all CU tests in sequence
#[tokio::test]
async fn test_all_cu_operations_comprehensive() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Test all operations in a single transaction to see cumulative CU usage
    let instructions = vec![
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestBasicArithmetic {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestSqrtVariants {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestTickConversions {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestMulDivVariants {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestLiquidityCalculations {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestBatchOperations {}.data(),
        },
        Instruction {
            program_id,
            accounts: vec![],
            data: fluxa_core::instruction::TestEdgeCases {}.data(),
        },
    ];

    // Execute each instruction separately to avoid CU limits
    for (i, instruction) in instructions.into_iter().enumerate() {
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        let result = banks_client.process_transaction(transaction).await;
        assert!(
            result.is_ok(),
            "Comprehensive CU test {} failed: {:?}",
            i,
            result
        );
    }
}

// Helper function to run a single CU test with detailed error handling
async fn run_cu_test(
    banks_client: &mut BanksClient,
    payer: &Keypair,
    recent_blockhash: solana_sdk::hash::Hash,
    program_id: Pubkey,
    instruction_data: Vec<u8>,
    test_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let instruction = Instruction {
        program_id,
        accounts: vec![],
        data: instruction_data,
    };

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client
        .process_transaction(transaction)
        .await
        .map_err(|e| format!("{} failed: {:?}", test_name, e).into())
}

// Performance benchmark test
#[tokio::test]
async fn benchmark_cu_performance() {
    let program_id = Pubkey::from_str(PROGRAM_ID).unwrap();
    let mut program_test =
        ProgramTest::new("fluxa_core", program_id, processor!(fluxa_core::entry));
    program_test.set_compute_max_units(1_400_000);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Run each test multiple times to get average performance
    let test_cases = vec![
        (
            "basic_arithmetic",
            fluxa_core::instruction::TestBasicArithmetic {}.data(),
        ),
        (
            "sqrt_variants",
            fluxa_core::instruction::TestSqrtVariants {}.data(),
        ),
        (
            "tick_conversions",
            fluxa_core::instruction::TestTickConversions {}.data(),
        ),
        (
            "mul_div_variants",
            fluxa_core::instruction::TestMulDivVariants {}.data(),
        ),
        (
            "liquidity_calculations",
            fluxa_core::instruction::TestLiquidityCalculations {}.data(),
        ),
        (
            "batch_operations",
            fluxa_core::instruction::TestBatchOperations {}.data(),
        ),
        (
            "edge_cases",
            fluxa_core::instruction::TestEdgeCases {}.data(),
        ),
    ];

    for (test_name, instruction_data) in test_cases {
        println!("Running benchmark for: {}", test_name);

        // Run test 3 times for consistency
        for run in 1..=3 {
            let result = run_cu_test(
                &mut banks_client,
                &payer,
                recent_blockhash,
                program_id,
                instruction_data.clone(),
                &format!("{}_{}", test_name, run),
            )
            .await;

            assert!(
                result.is_ok(),
                "Benchmark run {} for {} failed: {:?}",
                run,
                test_name,
                result
            );
        }
    }
}
