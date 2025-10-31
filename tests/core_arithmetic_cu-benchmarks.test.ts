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

describe("compute unit benchmarks", () => {
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

  const benches: {
    name: string;
    scenarios: {
      label: string;
      description: string;
      invoke: () => Promise<string>;
    }[];
  }[] = [
    {
      name: "mul_div",
      scenarios: [
        {
          label: "best_case",
          description: "Tiny swap settling nearly identical vault balances",
          invoke: () =>
            program.methods
              .benchmarkMulDiv({
                a: new BN(1_000_000),
                b: new BN(500_000),
                c: new BN(1_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Standard LP notional across active ticks",
          invoke: () =>
            program.methods
              .benchmarkMulDiv({
                a: new BN("1000000000000"),
                b: new BN("500000000000"),
                c: new BN("1000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "High TVL rebalance approaching upper precision bounds",
          invoke: () =>
            program.methods
              .benchmarkMulDiv({
                a: new BN("100000000000000000000"),
                b: new BN("75000000000000000000"),
                c: new BN("1000000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "mul_div_round_up",
      scenarios: [
        {
          label: "best_case",
          description: "Fee rounding with tiny token amounts",
          invoke: () =>
            program.methods
              .benchmarkMulDivRoundUp({
                a: new BN(1_000_000),
                b: new BN(500_000),
                c: new BN(1_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Rounding whole basis-point adjustments",
          invoke: () =>
            program.methods
              .benchmarkMulDivRoundUp({
                a: new BN("1000000000000"),
                b: new BN("500000000000"),
                c: new BN("1000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Upper-tail fee sweep with whale liquidity",
          invoke: () =>
            program.methods
              .benchmarkMulDivRoundUp({
                a: new BN("100000000000000000000"),
                b: new BN("75000000000000000000"),
                c: new BN("1000000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "mul_div_q64",
      scenarios: [
        {
          label: "best_case",
          description: "Unit liquidity scaling",
          invoke: () =>
            program.methods
              .benchmarkMulDivQ64({
                aRaw: q64Int(1),
                bRaw: q64Int(1),
                cRaw: q64Int(1),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Mid-range concentrated liquidity adjustment",
          invoke: () =>
            program.methods
              .benchmarkMulDivQ64({
                aRaw: q64Ratio(5, 2),
                bRaw: q64Ratio(3, 2),
                cRaw: q64Ratio(9, 8),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Extreme leverage shift across a wide tick range",
          invoke: () =>
            program.methods
              .benchmarkMulDivQ64({
                aRaw: q64Int(5_000),
                bRaw: q64Ratio(7, 3),
                cRaw: q64Ratio(5, 4),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "sqrt_x64",
      scenarios: [
        {
          label: "best_case",
          description: "Spot price exactly at 1.0",
          invoke: () =>
            program.methods
              .benchmarkSqrtX64({
                value: q64Int(1),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Price discovery around a 6.25x premium",
          invoke: () =>
            program.methods
              .benchmarkSqrtX64({
                value: q64Ratio(25, 4),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Mega-cap asset climbing six orders of magnitude",
          invoke: () =>
            program.methods
              .benchmarkSqrtX64({
                value: ONE_X64.mul(new BN(1_000_000)),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "recip_q64x64_nearest",
      scenarios: [
        {
          label: "best_case",
          description: "Invert unity (balanced pools)",
          invoke: () =>
            (program.methods as Record<string, any>)
              .benchmarkRecipQ64X64Nearest({
                raw: q64Int(1),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Stablecoin premium oscillating around 1.5x",
          invoke: () =>
            (program.methods as Record<string, any>)
              .benchmarkRecipQ64X64Nearest({
                raw: q64Ratio(3, 2),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Highly imbalanced price 1e-6 of reference",
          invoke: () =>
            (program.methods as Record<string, any>)
              .benchmarkRecipQ64X64Nearest({
                raw: q64Ratio(1, 1_000_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "tick_to_sqrt_x64",
      scenarios: [
        {
          label: "best_case",
          description: "Exactly at reference tick",
          invoke: () =>
            program.methods
              .benchmarkTickToSqrtX64({
                tick: 0,
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Active LP tick range around 12% drift",
          invoke: () =>
            program.methods
              .benchmarkTickToSqrtX64({
                tick: 1_200,
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Frontier tick used for oracle sanity sweep",
          invoke: () =>
            program.methods
              .benchmarkTickToSqrtX64({
                tick: 200_000,
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "liquidity_from_amount_0",
      scenarios: [
        {
          label: "best_case",
          description: "Tight range market making near spot",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount0({
                sqrtA: q64Ratio(95, 100),
                sqrtB: q64Ratio(105, 100),
                amount0: new BN(1_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Standard price banding over 3.2x width",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount0({
                sqrtA: q64Int(1),
                sqrtB: q64Ratio(32, 10),
                amount0: new BN("1000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Wide volatility hedge spanning 0.1x to 100x",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount0({
                sqrtA: q64Ratio(1, 10),
                sqrtB: q64Ratio(100, 1),
                amount0: new BN("5000000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
    {
      name: "liquidity_from_amount_1",
      scenarios: [
        {
          label: "best_case",
          description: "Small quote-side provision near parity",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount1({
                sqrtA: q64Ratio(95, 100),
                sqrtB: q64Ratio(105, 100),
                amount1: new BN(1_000),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "common_case",
          description: "Balanced range order around 1.5x price",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount1({
                sqrtA: q64Int(1),
                sqrtB: q64Ratio(3, 2),
                amount1: new BN("1000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
        {
          label: "worst_case",
          description: "Defensive liquidity spanning two orders of magnitude",
          invoke: () =>
            program.methods
              .benchmarkLiquidityFromAmount1({
                sqrtA: q64Ratio(1, 50),
                sqrtB: q64Ratio(125, 1),
                amount1: new BN("7500000000000"),
              })
              .accounts(baseAccounts)
              .rpc(),
        },
      ],
    },
  ];

  it("captures compute unit consumption for core arithmetic helpers", async () => {
    const results: Array<{
      name: string;
      scenario: string;
      description: string;
      averageComputeUnits: number | null;
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

        results.push({
          name: bench.name,
          scenario: scenario.label,
          description: scenario.description,
          averageComputeUnits,
          samples,
        });
        const summary = samples
          .map((sample) => (sample.computeUnits ?? "n/a").toString())
          .join(", ");
        // eslint-disable-next-line no-console
        console.log(
          `CU benchmark ${bench.name} (${scenario.label}): avg=${
            averageComputeUnits ?? "n/a"
          } samples=[${summary}] signatures=${samples
            .map((sample) => sample.signature)
            .join(" | ")}`
        );
      }
    }

    const outputPath = path.join(outDir, "cu_benchmarks.json");
    fs.writeFileSync(outputPath, JSON.stringify(results, null, 2));

    expect(
      results.some((entry) => entry.averageComputeUnits !== null)
    ).to.equal(true);
  });
});
