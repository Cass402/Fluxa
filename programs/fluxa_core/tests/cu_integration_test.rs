use anchor_lang::prelude::*;
use anchor_lang::InstructionData;
use fluxa_core;
use solana_program_test::*;
use solana_sdk::{
    instruction::Instruction,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

#[tokio::test]
async fn test_fixed_mul_cu_measurement() {
    let program_id = fluxa_core::id();
    let program_test = ProgramTest::new("fluxa_core", program_id, None);

    let (mut banks_client, payer, recent_blockhash) = program_test.start().await;

    // Test single multiplication
    let ix = Instruction {
        program_id,
        accounts: vec![], // CuTest has no accounts
        data: fluxa_core::instruction::TestFixedMulCu {}.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    println!("=== Testing Single Fixed Point Multiplication ===");
    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Single multiplication test failed: {:?}",
        result
    );

    // Test multiple scenarios
    let ix = Instruction {
        program_id,
        accounts: vec![], // CuTest has no accounts
        data: fluxa_core::instruction::TestFixedMulScenarios {}.data(),
    };

    let transaction = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    println!("=== Testing Multiple Precision Scenarios ===");
    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "Multiple scenarios test failed: {:?}",
        result
    );
}
