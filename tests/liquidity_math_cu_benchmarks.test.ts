import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import * as anchor from "@coral-xyz/anchor";
import type { Program } from "@coral-xyz/anchor";
import BN from "bn.js";
import { expect } from "chai";
import type { FluxaCore } from "../target/types/fluxa_core.ts";

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const defaultProviderUrl = "http://127.0.0.1:8899";
const defaultWalletPath = path.resolve(
  __dirname,
  "..",
  "wallets",
  "wallet1.json"
);

if (!process.env.ANCHOR_PROVIDER_URL) {
  process.env.ANCHOR_PROVIDER_URL = defaultProviderUrl;
}

if (!process.env.ANCHOR_WALLET) {
  process.env.ANCHOR_WALLET = defaultWalletPath;
}

describe("liquidity_math compute unit benchmarks", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.FluxaCore as Program<FluxaCore>;
  const outDir = path.resolve(__dirname, "..", "out");
  const sampleCount = Math.max(
    1,
    Number.parseInt(process.env.CU_BENCH_SAMPLES ?? "5", 10)
  );
  const delayMs = Math.max(
    0,
    Number.parseInt(process.env.CU_BENCH_SLEEP_MS ?? "250", 10)
  );
  const baseAccounts = {
    signer: provider.wallet.publicKey,
  } as const;

  const ONE_X64 = new BN(1).ushln(64);
  const q64Int = (int: number) => ONE_X64.mul(new BN(int));
  const q64Ratio = (numerator: number, denominator: number) =>
    ONE_X64.mul(new BN(numerator)).div(new BN(denominator));

  // Helper to generate sqrt price from tick approximation (simplified)
  const sqrtPriceFromTick = (tick: number): BN => {
    // sqrt(1.0001^tick) ≈ 1.0001^(tick/2)
    // For small ticks: sqrt_price ≈ 1 + tick * 0.00005
    const base = q64Int(1);
    const adjustment = base.mul(new BN(tick)).div(new BN(20000));
    return base.add(adjustment);
  };

  const dummyPubkey = anchor.web3.Keypair.generate().publicKey;

  const benches: {
    name: string;
    scenarios: {
      label: string;
      description: string;
      invoke: () => Promise<string>;
    }[];
  }[] = [
    {
      name: "calculate_amount_0_delta",
      scenarios: [
        {
          label: "best_case",
          description: "Tight range, minimal price spread",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount0Delta({
                sqrtPriceLower: q64Ratio(99, 100),
                sqrtPriceUpper: q64Ratio(101, 100),
                liquidity: q64Int(1000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Standard concentrated liquidity range (10% width)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount0Delta({
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                liquidity: q64Int(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Wide range with high liquidity",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount0Delta({
                sqrtPriceLower: q64Ratio(1, 10),
                sqrtPriceUpper: q64Int(10),
                liquidity: ONE_X64.mul(new BN("5000000000")),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "calculate_amount_1_delta",
      scenarios: [
        {
          label: "best_case",
          description: "Tight range, minimal price spread",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount1Delta({
                sqrtPriceLower: q64Ratio(99, 100),
                sqrtPriceUpper: q64Ratio(101, 100),
                liquidity: q64Int(1000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Standard concentrated liquidity range (10% width)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount1Delta({
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                liquidity: q64Int(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Wide range with high liquidity",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmount1Delta({
                sqrtPriceLower: q64Ratio(1, 10),
                sqrtPriceUpper: q64Int(10),
                liquidity: ONE_X64.mul(new BN("5000000000")),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "calculate_amounts_piecewise",
      scenarios: [
        {
          label: "active_range",
          description: "Price within range (both tokens required)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmountsPiecewise({
                sqrtPriceCurrent: q64Ratio(105, 100),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                liquidity: q64Int(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "below_range",
          description: "Price below range (only token0 required)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmountsPiecewise({
                sqrtPriceCurrent: q64Ratio(95, 100),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                liquidity: q64Int(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "above_range",
          description: "Price above range (only token1 required)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmountsPiecewise({
                sqrtPriceCurrent: q64Ratio(12, 10),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                liquidity: q64Int(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case_active",
          description: "Wide range, high liquidity, mid-range price",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateAmountsPiecewise({
                sqrtPriceCurrent: q64Int(3),
                sqrtPriceLower: q64Ratio(1, 10),
                sqrtPriceUpper: q64Int(10),
                liquidity: ONE_X64.mul(new BN("5000000000")),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "calculate_liquidity",
      scenarios: [
        {
          label: "dual_sided_balanced",
          description: "Both tokens provided, balanced amounts",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateLiquidity({
                sqrtPriceCurrent: q64Ratio(105, 100),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                amount0: new BN(1_000_000),
                amount1: new BN(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "token0_only",
          description: "Only token0 provided (constrained by amount0)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateLiquidity({
                sqrtPriceCurrent: q64Ratio(105, 100),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                amount0: new BN(1_000_000),
                amount1: new BN(0),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "token1_only",
          description: "Only token1 provided (constrained by amount1)",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateLiquidity({
                sqrtPriceCurrent: q64Ratio(105, 100),
                sqrtPriceLower: q64Int(1),
                sqrtPriceUpper: q64Ratio(11, 10),
                amount0: new BN(0),
                amount1: new BN(1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case_imbalanced",
          description: "Large amounts, wide range, imbalanced",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculateLiquidity({
                sqrtPriceCurrent: q64Int(2),
                sqrtPriceLower: q64Ratio(1, 10),
                sqrtPriceUpper: q64Int(10),
                amount0: new BN("10000000000"),
                amount1: new BN("100000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "calculate_position_value",
      scenarios: [
        {
          label: "small_position",
          description: "Small position, moderate prices",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculatePositionValue({
                owner: dummyPubkey,
                tickLower: -1000,
                tickUpper: 1000,
                liquidity: q64Int(10_000),
                currentSqrtPrice: q64Ratio(105, 100),
                token0PriceUsd: new BN(100),
                token1PriceUsd: new BN(100),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Typical LP position with realistic pricing",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculatePositionValue({
                owner: dummyPubkey,
                tickLower: -5000,
                tickUpper: 5000,
                liquidity: q64Int(1_000_000),
                currentSqrtPrice: q64Int(1),
                token0PriceUsd: new BN(50_000),
                token1PriceUsd: new BN(1),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "high_value_position",
          description: "Large position with expensive assets",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculatePositionValue({
                owner: dummyPubkey,
                tickLower: -20000,
                tickUpper: 20000,
                liquidity: ONE_X64.mul(new BN("10000000000")),
                currentSqrtPrice: q64Int(5),
                token0PriceUsd: new BN(100_000),
                token1PriceUsd: new BN(50_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case_overflow_path",
          description: "Testing overflow protection with large values",
          invoke: () =>
            (program.methods as any)
              .benchmarkCalculatePositionValue({
                owner: dummyPubkey,
                tickLower: -100000,
                tickUpper: 100000,
                liquidity: ONE_X64.mul(new BN("5000000000000")),
                currentSqrtPrice: q64Int(10),
                token0PriceUsd: new BN(4_294_967_295), // u32::MAX
                token1PriceUsd: new BN(4_294_967_295),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
  ];

  it("captures compute unit consumption for liquidity_math functions", async () => {
    const results: Array<{
      name: string;
      scenario: string;
      description: string;
      averageComputeUnits: number | null;
      maxComputeUnits: number | null;
      minComputeUnits: number | null;
      samples: Array<{ computeUnits: number | null; signature: string }>;
    }> = [];
    fs.mkdirSync(outDir, { recursive: true });

    const airdropSig = await provider.connection.requestAirdrop(
      provider.wallet.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    const latestBlockhash = await provider.connection.getLatestBlockhash();
    await provider.connection.confirmTransaction(
      {
        signature: airdropSig,
        ...latestBlockhash,
      },
      "confirmed"
    );

    for (const bench of benches) {
      for (const scenario of bench.scenarios) {
        const samples: Array<{
          computeUnits: number | null;
          signature: string;
        }> = [];

        for (let run = 0; run < sampleCount; run += 1) {
          const signature = await scenario.invoke();
          if (delayMs > 0) {
            await sleep(delayMs);
          }
          const tx = await provider.connection.getTransaction(signature, {
            commitment: "confirmed",
            maxSupportedTransactionVersion: 0,
          });
          expect(tx, `transaction ${signature} should exist`).to.not.be.null;
          const computeUnits = tx!.meta?.computeUnitsConsumed ?? null;
          samples.push({ computeUnits, signature });
        }

        const validSamples = samples
          .map((sample) => sample.computeUnits)
          .filter((value): value is number => value !== null);

        const averageComputeUnits =
          validSamples.length > 0
            ? Math.round(
                validSamples.reduce((acc, value) => acc + value, 0) /
                  validSamples.length
              )
            : null;

        const maxComputeUnits =
          validSamples.length > 0 ? Math.max(...validSamples) : null;

        const minComputeUnits =
          validSamples.length > 0 ? Math.min(...validSamples) : null;

        results.push({
          name: bench.name,
          scenario: scenario.label,
          description: scenario.description,
          averageComputeUnits,
          maxComputeUnits,
          minComputeUnits,
          samples,
        });

        const summary = samples
          .map((sample) => (sample.computeUnits ?? "n/a").toString())
          .join(", ");

        // Verify CU consumption is under 50k (critical requirement)
        if (averageComputeUnits !== null) {
          expect(averageComputeUnits).to.be.lessThan(
            50_000,
            `${bench.name}/${scenario.label} exceeds 50k CU limit`
          );
        }

        // eslint-disable-next-line no-console
        console.log(
          `CU benchmark ${bench.name} (${scenario.label}): ` +
            `avg=${averageComputeUnits ?? "n/a"} ` +
            `min=${minComputeUnits ?? "n/a"} ` +
            `max=${maxComputeUnits ?? "n/a"} ` +
            `samples=[${summary}]`
        );
      }
    }

    const outputPath = path.join(outDir, "liquidity_math_cu_benchmarks.json");
    fs.writeFileSync(outputPath, JSON.stringify(results, null, 2));

    // Verify all functions are well under the 200k Solana limit
    const maxCU = Math.max(
      ...results
        .map((r) => r.maxComputeUnits)
        .filter((v): v is number => v !== null)
    );

    expect(maxCU).to.be.lessThan(
      200_000,
      "At least one function exceeded Solana's 200k CU limit"
    );

    // Verify we captured data
    expect(
      results.some((entry) => entry.averageComputeUnits !== null)
    ).to.equal(true, "No compute unit data was captured");

    // Generate summary report
    // eslint-disable-next-line no-console
    console.log("\n=== LIQUIDITY_MATH CU BENCHMARK SUMMARY ===");
    // eslint-disable-next-line no-console
    console.log(`Total scenarios tested: ${results.length}`);
    // eslint-disable-next-line no-console
    console.log(`Maximum CU observed: ${maxCU}`);
    // eslint-disable-next-line no-console
    console.log(`All functions under 50k CU: ${maxCU < 50_000 ? "✅" : "❌"}`);
    // eslint-disable-next-line no-console
    console.log(
      `All functions under 200k CU: ${maxCU < 200_000 ? "✅" : "❌"}`
    );
    // eslint-disable-next-line no-console
    console.log("==========================================\n");
  });
});
