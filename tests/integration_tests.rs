//! Integration tests for Cliptions core functionality
//! 
//! These tests verify that all components work together correctly
//! and provide end-to-end testing of the complete system.

use tempfile::NamedTempFile;

use cliptions_core::commitment::{CommitmentGenerator, CommitmentVerifier};
use cliptions_core::embedder::{MockEmbedder, EmbedderTrait};
use cliptions_core::scoring::{ClipBatchStrategy, ScoreValidator, ScoringStrategy, calculate_rankings, calculate_payouts};
use cliptions_core::round::RoundProcessor;
use cliptions_core::types::{RoundData, Participant, Guess, RoundConfig, RoundStatus};

#[test]
fn test_complete_round_lifecycle() {
    // Create a temporary rounds file
    let temp_file = NamedTempFile::new().unwrap();
    let file_path = temp_file.path().to_string_lossy().to_string();
    
    // Create round processor
    let embedder = MockEmbedder::clip_like();
    let strategy = ClipBatchStrategy::new();
    let mut processor = RoundProcessor::new(file_path.clone(), embedder, strategy);
    
    // 1. Create a new round
    let round_id = "integration_test_round";
    processor.create_round(
        round_id.to_string(),
        "Integration Test Round".to_string(),
        "A round for testing the complete lifecycle".to_string(),
        "test_image.jpg".to_string(),
        Some(RoundConfig {
            prize_pool: 1000.0,
            max_guess_length: 300,
            scoring_version: "v0.3".to_string(),
        }),
    ).unwrap();
    
    // 2. Add participants with commitments
    let commitment_gen = CommitmentGenerator::new();
    let participants_data = vec![
        ("alice", "A beautiful sunset over the ocean", "salt_alice"),
        ("bob", "Mountains covered in snow", "salt_bob"),
        ("charlie", "A bustling city street at night", "salt_charlie"),
        ("diana", "Flowers blooming in spring", "salt_diana"),
    ];
    
    for (user_id, guess_text, salt) in &participants_data {
        let commitment = commitment_gen.generate(guess_text, salt).unwrap();
        let participant = Participant::new(
            user_id.to_string(),
            format!("user_{}", user_id),
            Guess::new(guess_text.to_string()),
            commitment,
        ).with_salt(salt.to_string());
        
        processor.add_participant(round_id, participant).unwrap();
    }
    
    // 3. Verify commitments
    let verification_results = processor.verify_commitments(round_id).unwrap();
    assert_eq!(verification_results.len(), 4);
    assert!(verification_results.iter().all(|&r| r)); // All should be valid
    
    // 4. Check round status before processing
    let round = processor.get_round(round_id).unwrap();
    assert_eq!(round.status, RoundStatus::Open);
    assert_eq!(round.verified_participants().len(), 4);
    
    // Note: We can't actually process payouts in the test because the image file doesn't exist
    // But we can test all the other components
    
    // 5. Test round statistics
    let stats = processor.get_round_stats(round_id).unwrap();
    assert_eq!(stats.round_id, round_id);
    assert_eq!(stats.total_participants, 4);
    assert_eq!(stats.verified_participants, 4);
    assert_eq!(stats.total_prize_pool, 1000.0);
    assert!(!stats.is_complete);
}

#[test]
fn test_commitment_system_integration() {
    let generator = CommitmentGenerator::new();
    let verifier = CommitmentVerifier::new();
    
    // Test multiple commitments
    let test_cases = vec![
        ("Hello, World!", "salt123"),
        ("Another message", "different_salt"),
        ("Special characters: !@#$%^&*()", "unicode_salt_🎉"),
        ("", "empty_message_salt"), // Edge case: empty message (should fail)
    ];
    
    for (message, salt) in test_cases {
        if message.is_empty() {
            // Empty message should fail
            assert!(generator.generate(message, salt).is_err());
            continue;
        }
        
        let commitment = generator.generate(message, salt).unwrap();
        
        // Valid verification
        assert!(verifier.verify(message, salt, &commitment));
        
        // Invalid verifications
        assert!(!verifier.verify("wrong message", salt, &commitment));
        assert!(!verifier.verify(message, "wrong_salt", &commitment));
        assert!(!verifier.verify(message, salt, "wrong_commitment"));
    }
}



#[test]
fn test_score_validator_integration() {
    let embedder = MockEmbedder::new(128);
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    
    // Test various guess validations
    let long_string = "x".repeat(400);
    let test_guesses = vec![
        ("Valid guess", true),
        ("", false), // Empty
        ("   ", false), // Whitespace only
        (long_string.as_str(), false), // Too long
        ("Normal length guess with punctuation!", true),
        ("Numbers 123 and symbols @#$", true),
    ];
    
    for (guess, expected_valid) in test_guesses {
        assert_eq!(validator.validate_guess(guess), expected_valid);
    }
    
    // Test batch similarity calculation
    let guesses = vec!["valid guess".to_string()];
    let similarities = validator.calculate_batch_similarities("test.jpg", &guesses).unwrap();
    assert_eq!(similarities.len(), 1);
    assert!(similarities[0] >= 0.0); // Should be non-negative for CLIP batch
}

#[test]
fn test_ranking_and_payout_calculation() {
    let embedder = MockEmbedder::new(128);
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    
    let guesses = vec![
        "First guess".to_string(),
        "Second guess".to_string(),
        "Third guess".to_string(),
        "Fourth guess".to_string(),
    ];
    
    // Calculate rankings
    let rankings = calculate_rankings("test.jpg", &guesses, &validator).unwrap();
    assert_eq!(rankings.len(), 4);
    
    // Should be sorted by score (highest first)
    for i in 1..rankings.len() {
        assert!(rankings[i-1].1 >= rankings[i].1);
    }
    
    // Calculate payouts
    let payouts = calculate_payouts(&rankings, 100.0).unwrap();
    assert_eq!(payouts.len(), 4);
    
    // Total payout should equal prize pool
    let total: f64 = payouts.iter().sum();
    assert!((total - 100.0).abs() < 1e-10);
    
    // First place should get the most
    assert!(payouts[0] >= payouts[1]);
    assert!(payouts[1] >= payouts[2]);
    assert!(payouts[2] >= payouts[3]);
}

#[test]
fn test_payout_calculation_with_ties() {
    // Test payout calculation when there are tied scores
    let ranked_results = vec![
        ("winner".to_string(), 0.95),
        ("tied_second_a".to_string(), 0.80),
        ("tied_second_b".to_string(), 0.80),
        ("tied_second_c".to_string(), 0.80),
        ("last".to_string(), 0.60),
    ];
    
    let payouts = calculate_payouts(&ranked_results, 1000.0).unwrap();
    assert_eq!(payouts.len(), 5);
    
    // Total should equal prize pool
    let total: f64 = payouts.iter().sum();
    assert!((total - 1000.0).abs() < 1e-10);
    
    // Tied positions should have equal payouts
    assert!((payouts[1] - payouts[2]).abs() < 1e-10);
    assert!((payouts[2] - payouts[3]).abs() < 1e-10);
    
    // Winner should get most, tied group should get equal amounts, last should get least
    assert!(payouts[0] > payouts[1]);
    assert!(payouts[1] > payouts[4]);
}

#[test]
fn test_batch_commitment_verification() {
    let generator = CommitmentGenerator::new();
    let verifier = CommitmentVerifier::new();
    
    // Create a batch of commitments
    let mut commitments = Vec::new();
    for i in 0..50 {
        let message = format!("Message {}", i);
        let salt = format!("salt_{}", i);
        let commitment = generator.generate(&message, &salt).unwrap();
        commitments.push((message, salt, commitment));
    }
    
    // Add some invalid commitments
    commitments.push(("valid message".to_string(), "valid_salt".to_string(), "invalid_commitment".to_string()));
    commitments.push(("another message".to_string(), "another_salt".to_string(), "also_invalid".to_string()));
    
    // Convert to references for verification
    let commitment_refs: Vec<(&str, &str, &str)> = commitments
        .iter()
        .map(|(m, s, c)| (m.as_str(), s.as_str(), c.as_str()))
        .collect();
    
    // Test sequential verification
    let sequential_results = verifier.verify_batch(&commitment_refs);
    
    // Test parallel verification
    let parallel_results = verifier.verify_batch_parallel(&commitment_refs);
    
    // Results should be identical
    assert_eq!(sequential_results, parallel_results);
    
    // First 50 should be valid, last 2 should be invalid
    assert_eq!(sequential_results.len(), 52);
    assert!(sequential_results[..50].iter().all(|&r| r));
    assert!(!sequential_results[50]);
    assert!(!sequential_results[51]);
}

#[test]
fn test_round_data_serialization() {
    // Test that round data can be properly serialized and deserialized
    let mut round = RoundData::new(
        "test_round".to_string(),
        "Test Round".to_string(),
        "A test round for serialization".to_string(),
        "test.jpg".to_string(),
    );
    
    // Add a participant
    let commitment_gen = CommitmentGenerator::new();
    let salt = "test_salt";
    let message = "test guess";
    let commitment = commitment_gen.generate(message, salt).unwrap();
    
    let participant = Participant::new(
        "user1".to_string(),
        "user_user1".to_string(),
        Guess::new(message.to_string()),
        commitment,
    ).with_salt(salt.to_string()).mark_verified();
    
    round.add_participant(participant);
    
    // Serialize to JSON
    let json_str = serde_json::to_string_pretty(&round).unwrap();
    
    // Deserialize back
    let deserialized_round: RoundData = serde_json::from_str(&json_str).unwrap();
    
    // Should be identical
    assert_eq!(round.round_id, deserialized_round.round_id);
    assert_eq!(round.title, deserialized_round.title);
    assert_eq!(round.participants.len(), deserialized_round.participants.len());
    assert_eq!(round.participants[0].user_id, deserialized_round.participants[0].user_id);
    assert_eq!(round.participants[0].verified, deserialized_round.participants[0].verified);
}

#[test]
fn test_error_handling() {
    // Test various error conditions
    
    // 1. Invalid prize pool
    let ranked_results = vec![("test".to_string(), 0.5)];
    assert!(calculate_payouts(&ranked_results, -10.0).is_err());
    assert!(calculate_payouts(&ranked_results, 0.0).is_err());
    
    // 2. Empty guesses
    let embedder = MockEmbedder::new(128);
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    assert!(calculate_rankings("test.jpg", &[], &validator).is_err());
    
    // 3. Empty salt for commitment
    let generator = CommitmentGenerator::new();
    assert!(generator.generate("message", "").is_err());
    
    // 4. Nonexistent round
    let temp_file = NamedTempFile::new().unwrap();
    let file_path = temp_file.path().to_string_lossy().to_string();
    let embedder = MockEmbedder::clip_like();
    let strategy = ClipBatchStrategy::new();
    let mut processor = RoundProcessor::new(file_path, embedder, strategy);
    
    assert!(processor.get_round("nonexistent").is_err());
}

#[test]
fn test_performance_characteristics() {
    // Test that operations scale reasonably with input size
    use std::time::Instant;
    
    let embedder = MockEmbedder::new(512); // CLIP-like dimensions
    let strategy = ClipBatchStrategy::new();
    let validator = ScoreValidator::new(embedder, strategy);
    
    // Test with different numbers of guesses
    for &num_guesses in &[10, 50, 100] {
        let guesses: Vec<String> = (0..num_guesses)
            .map(|i| format!("Guess number {}", i))
            .collect();
        
        let start = Instant::now();
        let rankings = calculate_rankings("test.jpg", &guesses, &validator).unwrap();
        let ranking_time = start.elapsed();
        
        let start = Instant::now();
        let _payouts = calculate_payouts(&rankings, 1000.0).unwrap();
        let payout_time = start.elapsed();
        
        println!("Guesses: {}, Ranking time: {:?}, Payout time: {:?}", 
                 num_guesses, ranking_time, payout_time);
        
        // Sanity checks
        assert_eq!(rankings.len(), num_guesses);
        
        // Performance should be reasonable (these are very loose bounds)
        assert!(ranking_time.as_millis() < 1000); // Should be under 1 second
        assert!(payout_time.as_millis() < 100);   // Should be under 100ms
    }
}

#[test]
fn test_concurrent_operations() {
    use std::sync::Arc;
    use std::thread;
    
    // Test that operations are thread-safe
    let generator = Arc::new(CommitmentGenerator::new());
    let verifier = Arc::new(CommitmentVerifier::new());
    
    let mut handles = Vec::new();
    
    // Spawn multiple threads doing commitment operations
    for i in 0..10 {
        let gen = Arc::clone(&generator);
        let ver = Arc::clone(&verifier);
        
        let handle = thread::spawn(move || {
            let message = format!("Message from thread {}", i);
            let salt = format!("salt_{}", i);
            
            let commitment = gen.generate(&message, &salt).unwrap();
            assert!(ver.verify(&message, &salt, &commitment));
        });
        
        handles.push(handle);
    }
    
    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_deterministic_behavior() {
    // Test that operations are deterministic
    let embedder = MockEmbedder::new(128);
    
    // Same input should always produce same output
    let text = "deterministic test";
    let embedding1 = embedder.get_text_embedding(text).unwrap();
    let embedding2 = embedder.get_text_embedding(text).unwrap();
    
    assert_eq!(embedding1, embedding2);
    
    // Same for commitments
    let generator = CommitmentGenerator::new();
    let commitment1 = generator.generate("message", "salt").unwrap();
    let commitment2 = generator.generate("message", "salt").unwrap();
    
    assert_eq!(commitment1, commitment2);
}