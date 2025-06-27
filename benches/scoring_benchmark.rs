//! Benchmarks for Cliptions core functionality
//! 
//! These benchmarks measure the performance of key operations to demonstrate
//! the performance improvements over the Python implementation.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;

use cliptions_core::commitment::{CommitmentGenerator, CommitmentVerifier};
use cliptions_core::embedder::MockEmbedder;
use cliptions_core::scoring::{ClipBatchStrategy, ScoreValidator, calculate_rankings, calculate_payouts};

fn benchmark_commitment_generation(c: &mut Criterion) {
    let generator = CommitmentGenerator::new();
    
    c.bench_function("commitment_generation", |b| {
        b.iter(|| {
            let message = black_box("test message for commitment");
            let salt = black_box("test_salt_value");
            generator.generate(message, salt).unwrap()
        })
    });
}

fn benchmark_commitment_verification(c: &mut Criterion) {
    let generator = CommitmentGenerator::new();
    let verifier = CommitmentVerifier::new();
    
    let message = "test message for verification";
    let salt = "test_salt_value";
    let commitment = generator.generate(message, salt).unwrap();
    
    c.bench_function("commitment_verification", |b| {
        b.iter(|| {
            verifier.verify(
                black_box(message),
                black_box(salt),
                black_box(&commitment)
            )
        })
    });
}

fn benchmark_batch_commitment_verification(c: &mut Criterion) {
    let generator = CommitmentGenerator::new();
    let verifier = CommitmentVerifier::new();
    
    let mut group = c.benchmark_group("batch_commitment_verification");
    
    for &size in &[10, 50, 100, 500] {
        // Prepare test data
        let mut commitments = Vec::new();
        for i in 0..size {
            let message = format!("Message {}", i);
            let salt = format!("salt_{}", i);
            let commitment = generator.generate(&message, &salt).unwrap();
            commitments.push((message, salt, commitment));
        }
        
        let commitment_refs: Vec<(&str, &str, &str)> = commitments
            .iter()
            .map(|(m, s, c)| (m.as_str(), s.as_str(), c.as_str()))
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("sequential", size),
            &commitment_refs,
            |b, refs| {
                b.iter(|| verifier.verify_batch(black_box(refs)))
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("parallel", size),
            &commitment_refs,
            |b, refs| {
                b.iter(|| verifier.verify_batch_parallel(black_box(refs)))
            },
        );
    }
    
    group.finish();
}

fn benchmark_embedding_generation(c: &mut Criterion) {
    let embedder = MockEmbedder::clip_like();
    
    c.bench_function("text_embedding", |b| {
        b.iter(|| {
            embedder.get_text_embedding(black_box("test text for embedding")).unwrap()
        })
    });
    
    c.bench_function("image_embedding", |b| {
        b.iter(|| {
            embedder.get_image_embedding(black_box("test_image.jpg")).unwrap()
        })
    });
}

fn benchmark_scoring_strategies(c: &mut Criterion) {
    let embedder = MockEmbedder::new(512);
    let strategy = ClipBatchStrategy::new();
    
    let image_features = embedder.get_image_embedding("test.jpg").unwrap();
    let text_features = embedder.get_text_embedding("test text").unwrap();
    
    c.bench_function("clip_batch_scoring", |b| {
        b.iter(|| {
            strategy.calculate_score(
                black_box(&image_features),
                black_box(&text_features),
            ).unwrap()
        })
    });
}

fn benchmark_ranking_calculation(c: &mut Criterion) {
    let embedder = MockEmbedder::clip_like();
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    
    let mut group = c.benchmark_group("ranking_calculation");
    group.measurement_time(Duration::from_secs(10));
    
    for &num_guesses in &[10, 25, 50, 100] {
        let guesses: Vec<String> = (0..num_guesses)
            .map(|i| format!("Guess number {} with some descriptive text", i))
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("calculate_rankings", num_guesses),
            &guesses,
            |b, guesses| {
                b.iter(|| {
                    calculate_rankings(
                        black_box("test_image.jpg"),
                        black_box(guesses),
                        black_box(&validator)
                    ).unwrap()
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_payout_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("payout_calculation");
    
    for &num_participants in &[10, 25, 50, 100, 500] {
        // Create ranked results with some ties
        let mut ranked_results = Vec::new();
        for i in 0..num_participants {
            let score = 1.0 - (i as f64 / num_participants as f64) * 0.5;
            // Create some ties every 5 positions
            let adjusted_score = if i % 5 == 0 && i > 0 {
                ranked_results[i-1].1 // Same as previous
            } else {
                score
            };
            ranked_results.push((format!("participant_{}", i), adjusted_score));
        }
        
        group.bench_with_input(
            BenchmarkId::new("calculate_payouts", num_participants),
            &ranked_results,
            |b, results| {
                b.iter(|| {
                    calculate_payouts(black_box(results), black_box(1000.0)).unwrap()
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_complete_scoring_pipeline(c: &mut Criterion) {
    let embedder = MockEmbedder::clip_like();
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    
    let mut group = c.benchmark_group("complete_scoring_pipeline");
    group.measurement_time(Duration::from_secs(15));
    
    for &num_guesses in &[10, 25, 50] {
        let guesses: Vec<String> = (0..num_guesses)
            .map(|i| format!("Guess {} with descriptive content about the image", i))
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("full_pipeline", num_guesses),
            &guesses,
            |b, guesses| {
                b.iter(|| {
                    // Complete pipeline: ranking + payout calculation
                    let rankings = calculate_rankings(
                        black_box("test_image.jpg"),
                        black_box(guesses),
                        black_box(&validator)
                    ).unwrap();
                    
                    let _payouts = calculate_payouts(
                        black_box(&rankings),
                        black_box(1000.0)
                    ).unwrap();
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_memory_efficiency(c: &mut Criterion) {
    // Test memory allocation patterns for large datasets
    let mut group = c.benchmark_group("memory_efficiency");
    
    for &num_guesses in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("large_guess_allocation", num_guesses),
            &num_guesses,
            |b, &size| {
                b.iter(|| {
                    let guesses: Vec<String> = (0..size)
                        .map(|i| format!("Guess {} with substantial text content to test memory allocation patterns and performance characteristics", i))
                        .collect();
                    black_box(guesses)
                })
            },
        );
    }
    
    group.finish();
}

fn benchmark_concurrent_operations(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;
    
    let generator = Arc::new(CommitmentGenerator::new());
    let verifier = Arc::new(CommitmentVerifier::new());
    
    c.bench_function("concurrent_commitment_operations", |b| {
        b.iter(|| {
            let gen = Arc::clone(&generator);
            let ver = Arc::clone(&verifier);
            
            let handles: Vec<_> = (0..4).map(|i| {
                let gen = Arc::clone(&gen);
                let ver = Arc::clone(&ver);
                
                thread::spawn(move || {
                    let message = format!("Message {}", i);
                    let salt = format!("salt_{}", i);
                    
                    let commitment = gen.generate(&message, &salt).unwrap();
                    ver.verify(&message, &salt, &commitment)
                })
            }).collect();
            
            for handle in handles {
                black_box(handle.join().unwrap());
            }
        })
    });
}

criterion_group!(
    benches,
    benchmark_commitment_generation,
    benchmark_commitment_verification,
    benchmark_batch_commitment_verification,
    benchmark_embedding_generation,
    benchmark_scoring_strategies,
    benchmark_ranking_calculation,
    benchmark_payout_calculation,
    benchmark_complete_scoring_pipeline,
    benchmark_memory_efficiency,
    benchmark_concurrent_operations
);

criterion_main!(benches);