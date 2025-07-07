use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use fluxa_core::math::core_arithmetic::*;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::hint::black_box;

// ========== Core Math Benchmarks ==========

fn bench_q64_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("q64_arithmetic");
    group.throughput(Throughput::Elements(1));

    // Test data sets
    let small_values = [
        Q64x64::from_int(1),
        Q64x64::from_raw(ONE_X64 / 2), // 0.5
        Q64x64::from_raw(ONE_X64 * 2), // 2.0
    ];

    let large_values = [
        Q64x64::from_raw(u128::MAX / 4),
        Q64x64::from_raw(u128::MAX / 2),
        Q64x64::from_raw(MAX_SQRT_X64),
    ];

    let edge_values = [
        Q64x64::zero(),
        Q64x64::from_raw(1),           // Smallest non-zero
        Q64x64::from_raw(1u128 << 32), // Very small fraction
    ];

    // Benchmark addition
    for (name, values) in [
        ("small", &small_values[..]),
        ("large", &large_values[..]),
        ("edge", &edge_values[..]),
    ] {
        group.bench_with_input(BenchmarkId::new("add", name), values, |b, vals| {
            b.iter(|| {
                for i in 0..vals.len() {
                    for j in 0..vals.len() {
                        let _ = black_box(vals[i].checked_add(vals[j]));
                    }
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("sub", name), values, |b, vals| {
            b.iter(|| {
                for i in 0..vals.len() {
                    for j in 0..vals.len() {
                        let _ = black_box(vals[i].checked_sub(vals[j]));
                    }
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("mul", name), values, |b, vals| {
            b.iter(|| {
                for i in 0..vals.len() {
                    for j in 0..vals.len() {
                        let _ = black_box(vals[i].checked_mul(vals[j]));
                    }
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("div", name), values, |b, vals| {
            b.iter(|| {
                for i in 0..vals.len() {
                    for j in 0..vals.len() {
                        if vals[j].raw() != 0 {
                            let _ = black_box(vals[i].checked_div(vals[j]));
                        }
                    }
                }
            });
        });
    }

    // Same-value operations (x + x, x / x, etc.)
    group.bench_function("self_operations", |b| {
        let val = Q64x64::from_int(42);
        b.iter(|| {
            let _ = black_box(val.checked_add(val));
            let _ = black_box(val.checked_sub(val));
            let _ = black_box(val.checked_mul(val));
            let _ = black_box(val.checked_div(val));
        });
    });

    group.finish();
}

fn bench_mul_div(c: &mut Criterion) {
    let mut group = c.benchmark_group("mul_div");
    group.throughput(Throughput::Elements(1));

    // Reproducible random values using seeded RNG
    let mut rng = StdRng::seed_from_u64(42);
    let test_cases: Vec<(u128, u128, u128)> = (0..100)
        .map(|_| {
            let a = rng.random_range(1..=1_000_000u128);
            let b = rng.random_range(1..=1_000_000u128);
            let c = rng.random_range(1..=1_000_000u128);
            (a, b, c)
        })
        .collect();

    group.bench_function("mul_div_batch", |b| {
        b.iter_batched(
            || test_cases.clone(),
            |cases| {
                for (a, b, c) in cases {
                    let _ = black_box(mul_div(a, b, c));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("mul_div_round_up_batch", |b| {
        b.iter_batched(
            || test_cases.clone(),
            |cases| {
                for (a, b, c) in cases {
                    let _ = black_box(mul_div_round_up(a, b, c));
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Compare normal vs round-up on same inputs
    group.bench_function("mul_div_comparison", |bencher| {
        let (a, b, c) = (123456u128, 789012u128, 345678u128);
        bencher.iter(|| {
            let normal = black_box(mul_div(a, b, c));
            let round_up = black_box(mul_div_round_up(a, b, c));
            black_box((normal, round_up))
        });
    });

    // Edge cases: large values
    group.bench_function("mul_div_large_values", |b| {
        let cases = [
            (u128::MAX / 4, u128::MAX / 4, u128::MAX / 2),
            (1u128 << 64, 1u128 << 32, 1u128 << 16),
            (MAX_SQRT_X64, ONE_X64, ONE_X64 * 2),
        ];
        b.iter(|| {
            for (a, b, c) in cases {
                let _ = black_box(mul_div(a, b, c));
            }
        });
    });

    group.finish();
}

fn bench_sqrt_x64(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqrt_x64");
    group.throughput(Throughput::Elements(1));

    // Test realistic inputs
    let test_inputs = [
        Q64x64::zero(),
        Q64x64::one(),
        Q64x64::from_raw(1u128 << 32),   // Very small fraction
        Q64x64::from_raw(ONE_X64 / 4),   // 0.25
        Q64x64::from_raw(ONE_X64 * 4),   // 4.0
        Q64x64::from_raw(ONE_X64 * 100), // 100.0
        Q64x64::from_raw(MIN_SQRT_X64),
        Q64x64::from_raw(MAX_SQRT_X64),
        Q64x64::from_raw(MAX_SQRT_X64 / 2),
    ];

    for (i, input) in test_inputs.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("single", format!("case_{i}")),
            input,
            |b, &val| {
                b.iter(|| black_box(sqrt_x64(val)));
            },
        );
    }

    // Batch processing
    group.bench_function("sqrt_batch", |b| {
        b.iter(|| {
            for input in test_inputs {
                let _ = black_box(sqrt_x64(input));
            }
        });
    });

    // Newton-Raphson performance on different magnitudes
    group.bench_function("sqrt_magnitude_sweep", |b| {
        let magnitudes: Vec<Q64x64> = (0..20)
            .map(|i| Q64x64::from_raw(ONE_X64 << i.min(63)))
            .collect();

        b.iter(|| {
            for mag in &magnitudes {
                let _ = black_box(sqrt_x64(*mag));
            }
        });
    });

    group.finish();
}

// ========== Tick Math Benchmarks ==========

fn bench_tick_to_sqrt(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_to_sqrt");
    group.throughput(Throughput::Elements(1));

    // Sweep across tick range in steps
    let tick_samples: Vec<i32> = (MIN_TICK..=MAX_TICK).step_by(5000).collect();

    group.bench_function("tick_sweep", |b| {
        b.iter(|| {
            for &tick in &tick_samples {
                let _ = black_box(tick_to_sqrt_x64(tick));
            }
        });
    });

    // Specific tick values
    let special_ticks = [MIN_TICK, -100_000, -1000, 0, 1000, 100_000, MAX_TICK];

    for &tick in &special_ticks {
        group.bench_with_input(BenchmarkId::new("single", tick), &tick, |b, &t| {
            b.iter(|| black_box(tick_to_sqrt_x64(t)));
        });
    }

    // Hot path: same tick repeatedly (cache behavior)
    group.bench_function("hot_tick_reuse", |b| {
        let hot_tick = 12345i32;
        b.iter(|| {
            for _ in 0..100 {
                let _ = black_box(tick_to_sqrt_x64(hot_tick));
            }
        });
    });

    group.finish();
}

// ========== Liquidity Benchmarks ==========

fn bench_liquidity_math(c: &mut Criterion) {
    let mut group = c.benchmark_group("liquidity");
    group.throughput(Throughput::Elements(1));

    // Test cases: (sqrt_a, sqrt_b, amount)
    let test_cases = [
        // Small amounts, narrow range
        (
            Q64x64::from_raw(MIN_SQRT_X64),
            Q64x64::from_raw(MIN_SQRT_X64 * 2),
            1000u64,
        ),
        // Medium amounts, medium range
        (Q64x64::from_int(1), Q64x64::from_int(2), 1_000_000u64),
        // Large amounts, wide range
        (Q64x64::from_int(10), Q64x64::from_int(100), u64::MAX / 1000),
        // Edge case: very close sqrt values
        (
            Q64x64::from_int(50),
            Q64x64::from_raw(Q64x64::from_int(50).raw() + 1000),
            500_000u64,
        ),
    ];

    for (i, &(sqrt_a, sqrt_b, amount)) in test_cases.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("liq_from_amount0", i),
            &(sqrt_a, sqrt_b, amount),
            |bencher, &(a, sqrt_b_val, amt)| {
                bencher.iter(|| black_box(liquidity_from_amount_0(a, sqrt_b_val, amt)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("liq_from_amount1", i),
            &(sqrt_a, sqrt_b, amount),
            |bencher, &(a, sqrt_b_val, amt)| {
                bencher.iter(|| black_box(liquidity_from_amount_1(a, sqrt_b_val, amt)));
            },
        );
    }

    // Batch processing both functions
    group.bench_function("liquidity_batch", |bencher| {
        bencher.iter(|| {
            for &(sqrt_a, sqrt_b, amount) in &test_cases {
                let _ = black_box(liquidity_from_amount_0(sqrt_a, sqrt_b, amount));
                let _ = black_box(liquidity_from_amount_1(sqrt_a, sqrt_b, amount));
            }
        });
    });

    group.finish();
}

// ========== Realistic Scenario Benchmarks ==========

fn bench_lp_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("lp_scenarios");
    group.throughput(Throughput::Elements(1));

    // Simulate LP entering random tick ranges
    let mut rng = StdRng::seed_from_u64(123);
    let lp_scenarios: Vec<(i32, i32, u64, u64)> = (0..50)
        .map(|_| {
            let tick_lower = rng.random_range(MIN_TICK + 1000..MAX_TICK - 1000);
            let tick_upper = tick_lower + rng.random_range(100..10000);
            let amount0 = rng.random_range(1000..1_000_000u64);
            let amount1 = rng.random_range(1000..1_000_000u64);
            (tick_lower, tick_upper, amount0, amount1)
        })
        .collect();

    group.bench_function("lp_enter_position", |b| {
        b.iter_batched(
            || lp_scenarios.clone(),
            |scenarios| {
                for (tick_lower, tick_upper, amount0, amount1) in scenarios {
                    // Convert ticks to sqrt prices
                    let sqrt_a = black_box(tick_to_sqrt_x64(tick_lower).unwrap());
                    let sqrt_b = black_box(tick_to_sqrt_x64(tick_upper).unwrap());

                    // Calculate liquidity from both amounts
                    let _ = black_box(liquidity_from_amount_0(sqrt_a, sqrt_b, amount0));
                    let _ = black_box(liquidity_from_amount_1(sqrt_a, sqrt_b, amount1));
                }
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_amm_core_flow(c: &mut Criterion) {
    let mut group = c.benchmark_group("amm_core_flow");
    group.throughput(Throughput::Elements(1));

    // End-to-end AMM math: sqrt -> mul_div -> liquidity
    group.bench_function("core_flow_pipeline", |b| {
        let input_value = Q64x64::from_int(42);
        let scale_factor = Q64x64::from_raw(ONE_X64 * 3 / 2); // 1.5x
        let amount = 100_000u64;

        b.iter(|| {
            // Step 1: Take square root
            let sqrt_val = black_box(sqrt_x64(input_value).unwrap());

            // Step 2: Scale using mul_div
            let scaled = black_box(mul_div_q64(sqrt_val, scale_factor, Q64x64::one()).unwrap());

            // Step 3: Calculate liquidity
            let sqrt_upper = black_box(Q64x64::from_raw(scaled.raw() * 2));
            let _ = black_box(liquidity_from_amount_1(scaled, sqrt_upper, amount));
        });
    });

    // Simulate swap calculation hot path
    group.bench_function("swap_math_hot_path", |b| {
        let current_tick = 12345i32;
        let amount_in = 50_000u64;

        b.iter(|| {
            // Convert current tick to sqrt price
            let sqrt_price = black_box(tick_to_sqrt_x64(current_tick).unwrap());

            // Scale by amount (simulating price impact)
            let scaled_price =
                black_box(mul_div(sqrt_price.raw(), amount_in as u128, 100_000u128).unwrap());

            // Take sqrt for final price
            let final_sqrt = black_box(sqrt_x64(Q64x64::from_raw(scaled_price)).unwrap());

            black_box(final_sqrt)
        });
    });

    group.finish();
}

fn bench_hot_path_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("hot_path_caching");
    group.throughput(Throughput::Elements(100)); // 100 operations per iter

    // Test caching behavior with repeated operations
    group.bench_function("tick_conversion_hot_reuse", |b| {
        let stable_tick = 50_000i32;
        b.iter(|| {
            for _ in 0..100 {
                let _ = black_box(tick_to_sqrt_x64(stable_tick));
            }
        });
    });

    group.bench_function("sqrt_hot_reuse", |b| {
        let stable_value = Q64x64::from_int(25);
        b.iter(|| {
            for _ in 0..100 {
                let _ = black_box(sqrt_x64(stable_value));
            }
        });
    });

    group.bench_function("mul_div_hot_reuse", |bencher| {
        let (a, b, c) = (123_456u128, 789_012u128, 345_678u128);
        bencher.iter(|| {
            for _ in 0..100 {
                let _ = black_box(mul_div(a, b, c));
            }
        });
    });

    group.finish();
}

// ========== Benchmark Groups ==========

criterion_group!(
    core_math,
    bench_q64_arithmetic,
    bench_mul_div,
    bench_sqrt_x64
);

criterion_group!(tick_math, bench_tick_to_sqrt);

criterion_group!(liquidity_math, bench_liquidity_math);

criterion_group!(realistic_scenarios, bench_lp_scenarios, bench_amm_core_flow);

criterion_group!(hot_path_simulation, bench_hot_path_caching);

criterion_main!(
    core_math,
    tick_math,
    liquidity_math,
    realistic_scenarios,
    hot_path_simulation
);
