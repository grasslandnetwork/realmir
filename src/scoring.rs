//! Scoring strategies and payout calculation for Cliptions
//! 
//! This module implements various scoring strategies for calculating similarity between
//! image and text embeddings, as well as payout calculation based on rankings.

use ndarray::Array1;
use std::sync::Arc;
use crate::embedder::EmbedderTrait;
use crate::error::{ScoringError, Result};
use crate::types::{Participant, ScoringResult};

/// Trait for scoring strategies
/// 
/// This corresponds to the Python IScoringStrategy interface
pub trait ScoringStrategy: Send + Sync {
    /// Calculate the similarity score between image and text features
    /// 
    /// # Arguments
    /// * `image_features` - The embedding vector for the image
    /// * `text_features` - The embedding vector for the text
    /// 
    /// # Returns
    /// The calculated similarity score
    fn calculate_score(
        &self,
        image_features: &Array1<f64>,
        text_features: &Array1<f64>,
    ) -> Result<f64>;
    
    /// Get the name of this scoring strategy
    fn name(&self) -> &str;
}

/// CLIP batch processing strategy
/// 
/// This strategy uses proper CLIP model.forward() with softmax to create competitive rankings.
/// Individual scoring is bypassed in favor of batch processing for correct results.
#[derive(Debug, Clone)]
pub struct ClipBatchStrategy;

impl ClipBatchStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClipBatchStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoringStrategy for ClipBatchStrategy {
    fn calculate_score(
        &self,
        _image_features: &Array1<f64>,
        _text_features: &Array1<f64>,
    ) -> Result<f64> {
        // This method is not used with ClipBatchStrategy
        // All scoring is done via calculate_batch_similarities for proper CLIP results
        Err(ScoringError::UnsupportedOperation.into())
    }
    
    fn name(&self) -> &str {
        "ClipBatch"
    }
}

/// Score validator for validating guesses and calculating scores
/// 
/// This corresponds to the Python ScoreValidator class
pub struct ScoreValidator<E: EmbedderTrait, S: ScoringStrategy> {
    embedder: Arc<E>,
    scoring_strategy: Arc<S>,
    max_tokens: usize,
}

impl<E: EmbedderTrait, S: ScoringStrategy> ScoreValidator<E, S> {
    /// Create a new score validator
    pub fn new(embedder: E, scoring_strategy: S) -> Self {
        Self {
            embedder: Arc::new(embedder),
            scoring_strategy: Arc::new(scoring_strategy),
            max_tokens: 77, // CLIP's maximum token limit
        }
    }
    
    /// Check if guess meets basic validity criteria
    pub fn validate_guess(&self, guess: &str) -> bool {
        // Check if guess is a string with content
        if guess.is_empty() || guess.trim().is_empty() {
            return false;
        }
        
        // CLIP can handle up to 77 tokens, but we'll estimate
        // Average token is ~4 characters in English, so ~300 chars
        // This is a rough estimate; the actual tokenizer would be more accurate
        if guess.len() > 300 {
            return false;
        }
        
        true
    }
    

    
    /// Get image embedding for a given image path
    /// 
    /// This is a convenience method for Python bindings
    pub fn get_image_embedding(&self, image_path: &str) -> Result<Array1<f64>> {
        self.embedder.get_image_embedding(image_path)
    }
    
    /// Calculate batch similarities using proper CLIP forward pass
    /// 
    /// This is the correct way to rank multiple texts against an image, as it uses
    /// the model's forward pass with softmax to create competitive rankings.
    /// 
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// * `guesses` - List of text guesses to rank
    /// 
    /// # Returns
    /// Vector of similarity scores (as percentages) in the same order as input guesses
    pub fn calculate_batch_similarities(&self, image_path: &str, guesses: &[String]) -> Result<Vec<f64>> {
        // Filter out invalid guesses and keep track of original indices
        let mut valid_guesses = Vec::new();
        let mut valid_indices = Vec::new();
        
        for (i, guess) in guesses.iter().enumerate() {
            if self.validate_guess(guess) {
                valid_guesses.push(guess.clone());
                valid_indices.push(i);
            }
        }
        
        if valid_guesses.is_empty() {
            // Return zeros for all invalid guesses
            return Ok(vec![0.0; guesses.len()]);
        }
        
        // Use the embedder's batch similarity calculation
        let valid_similarities = self.embedder.calculate_batch_similarities(image_path, &valid_guesses)?;
        
        // Map back to original positions
        let mut all_similarities = vec![0.0; guesses.len()];
        for (valid_idx, &original_idx) in valid_indices.iter().enumerate() {
            all_similarities[original_idx] = valid_similarities[valid_idx];
        }
        
        Ok(all_similarities)
    }
    
    /// Get raw batch similarities directly from embedder (for testing)
    /// 
    /// This bypasses the ScoreValidator's filtering and returns raw embedder results
    pub fn get_raw_batch_similarities(&self, image_path: &str, texts: &[String]) -> Result<Vec<f64>> {
        self.embedder.calculate_batch_similarities(image_path, texts)
    }
    
    /// Get image embedding (for testing)
    pub fn get_image_embedding_test(&self, image_path: &str) -> Result<Array1<f64>> {
        self.embedder.get_image_embedding(image_path)
    }
    
    /// Get text embedding (for testing)
    pub fn get_text_embedding_test(&self, text: &str) -> Result<Array1<f64>> {
        self.embedder.get_text_embedding(text)
    }
}

/// Calculate rankings for guesses based on similarity to target image
/// 
/// Uses proper CLIP batch processing with softmax for competitive rankings.
/// 
/// # Arguments
/// * `target_image_path` - Path to the target image
/// * `guesses` - List of text guesses to rank
/// * `validator` - Score validator to use
/// 
/// # Returns
/// List of tuples (guess, similarity) sorted by similarity (highest to lowest)
pub fn calculate_rankings<E: EmbedderTrait, S: ScoringStrategy>(
    target_image_path: &str,
    guesses: &[String],
    validator: &ScoreValidator<E, S>,
) -> Result<Vec<(String, f64)>> {
    if guesses.is_empty() {
        return Err(ScoringError::EmptyGuesses.into());
    }
    
    // Use the new batch similarity calculation (correct CLIP approach)
    let similarities = validator.calculate_batch_similarities(target_image_path, guesses)?;
    
    // Pair guesses with their similarities
    let mut paired_results: Vec<(String, f64)> = guesses.iter()
        .zip(similarities.iter())
        .map(|(guess, &sim)| (guess.clone(), sim))
        .collect();
    
    // Sort by similarity score (highest to lowest)
    paired_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    Ok(paired_results)
}

/// Calculate payouts based on rankings
/// 
/// The payout calculation uses a position-based scoring system where:
/// - Scores are based only on position (1st, 2nd, etc), not similarity values
/// - Equal similarity scores get equal payouts (ties split the combined payout)
/// - Scores sum to 1.0 to distribute full prize pool
/// - Higher positions get proportionally higher scores
/// 
/// # Arguments
/// * `ranked_results` - List of (guess, similarity) tuples sorted by similarity
/// * `prize_pool` - Total amount to distribute
/// 
/// # Returns
/// List of payouts corresponding to ranked_results
pub fn calculate_payouts(ranked_results: &[(String, f64)], prize_pool: f64) -> Result<Vec<f64>> {
    if prize_pool <= 0.0 {
        return Err(ScoringError::InvalidPrizePool { amount: prize_pool }.into());
    }
    
    if ranked_results.is_empty() {
        return Ok(Vec::new());
    }
    
    let total_guesses = ranked_results.len();
    let denominator: usize = (1..=total_guesses).sum();
    
    // Group positions by similarity score
    let mut groups = Vec::new();
    let mut current_group = Vec::new();
    let mut current_similarity: Option<f64> = None;
    
    for (guess, similarity) in ranked_results {
        match current_similarity {
            Some(sim) if (sim - similarity).abs() < f64::EPSILON => {
                current_group.push((guess.clone(), *similarity));
            }
            _ => {
                if !current_group.is_empty() {
                    groups.push(current_group);
                }
                current_group = vec![(guess.clone(), *similarity)];
                current_similarity = Some(*similarity);
            }
        }
    }
    
    if !current_group.is_empty() {
        groups.push(current_group);
    }
    
    // Calculate payouts
    let mut payouts = Vec::new();
    let mut position = 0;
    
    for group in groups {
        // Calculate total points for this group's positions
        let group_size = group.len();
        let group_points: usize = (0..group_size)
            .map(|i| total_guesses - (position + i))
            .sum();
        
        // Split points equally among tied positions
        let points_per_position = group_points as f64 / group_size as f64;
        let score = points_per_position / denominator as f64;
        
        // Add same payout for each tied position
        for _ in 0..group_size {
            payouts.push(score * prize_pool);
        }
        
        position += group_size;
    }
    
    Ok(payouts)
}

/// Process participants and calculate their scores and payouts
pub fn process_participants<E: EmbedderTrait, S: ScoringStrategy>(
    participants: &[Participant],
    target_image_path: &str,
    prize_pool: f64,
    validator: &ScoreValidator<E, S>,
) -> Result<Vec<ScoringResult>> {
    if participants.is_empty() {
        return Ok(Vec::new());
    }
    
    // Extract guesses
    let guesses: Vec<String> = participants.iter()
        .map(|p| p.guess.text.clone())
        .collect();
    
    // Calculate rankings
    let ranked_results = calculate_rankings(target_image_path, &guesses, validator)?;
    
    // Calculate payouts
    let payouts = calculate_payouts(&ranked_results, prize_pool)?;
    
    // Create scoring results
    let mut results = Vec::new();
    for (i, ((guess, score), payout)) in ranked_results.iter().zip(payouts.iter()).enumerate() {
        // Find the participant with this guess
        if let Some(participant) = participants.iter().find(|p| &p.guess.text == guess) {
            let result = ScoringResult::new(participant.clone(), *score)
                .with_adjusted_score(*score)
                .with_rank(i + 1)
                .with_payout(*payout);
            results.push(result);
        }
    }
    
    Ok(results)
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::MockEmbedder;
    
    #[test]
    fn test_clip_batch_strategy() {
        let strategy = ClipBatchStrategy::new();
        let embedder = MockEmbedder::new(128);
        
        let img_features = embedder.get_image_embedding("test.jpg").unwrap();
        let txt_features = embedder.get_text_embedding("test text").unwrap();
        
        // ClipBatchStrategy should return UnsupportedOperation for individual scoring
        let result = strategy.calculate_score(&img_features, &txt_features);
        assert!(matches!(result, Err(crate::error::CliptionsError::Scoring(ScoringError::UnsupportedOperation))));
    }
    
    #[test]
    fn test_score_validator() {
        let embedder = MockEmbedder::new(128);
        let strategy = ClipBatchStrategy::new();
        let validator = ScoreValidator::new(embedder, strategy);
        
        // Valid guess
        assert!(validator.validate_guess("valid guess"));
        
        // Invalid guesses
        assert!(!validator.validate_guess(""));
        assert!(!validator.validate_guess("   "));
        assert!(!validator.validate_guess(&"x".repeat(400))); // Too long
    }
    
    #[test]
    fn test_calculate_rankings() {
        let embedder = MockEmbedder::new(128);
        let strategy = ClipBatchStrategy::new();
        let validator = ScoreValidator::new(embedder, strategy);
        
        let guesses = vec![
            "guess1".to_string(),
            "guess2".to_string(),
            "guess3".to_string(),
        ];
        
        let rankings = calculate_rankings("test.jpg", &guesses, &validator).unwrap();
        
        assert_eq!(rankings.len(), 3);
        
        // Should be sorted by score (highest first)
        for i in 1..rankings.len() {
            assert!(rankings[i-1].1 >= rankings[i].1);
        }
    }
    
    #[test]
    fn test_calculate_payouts_no_ties() {
        let ranked_results = vec![
            ("first".to_string(), 0.9),
            ("second".to_string(), 0.7),
            ("third".to_string(), 0.5),
        ];
        
        let payouts = calculate_payouts(&ranked_results, 100.0).unwrap();
        
        assert_eq!(payouts.len(), 3);
        
        // Total should equal prize pool
        let total: f64 = payouts.iter().sum();
        assert!((total - 100.0).abs() < 1e-10);
        
        // First place should get the most
        assert!(payouts[0] > payouts[1]);
        assert!(payouts[1] > payouts[2]);
    }
    
    #[test]
    fn test_calculate_payouts_with_ties() {
        let ranked_results = vec![
            ("first".to_string(), 0.9),
            ("tied_second_a".to_string(), 0.7),
            ("tied_second_b".to_string(), 0.7),
            ("fourth".to_string(), 0.5),
        ];
        
        let payouts = calculate_payouts(&ranked_results, 100.0).unwrap();
        
        assert_eq!(payouts.len(), 4);
        
        // Total should equal prize pool
        let total: f64 = payouts.iter().sum();
        assert!((total - 100.0).abs() < 1e-10);
        
        // Tied positions should have equal payouts
        assert!((payouts[1] - payouts[2]).abs() < 1e-10);
        
        // First should be highest, tied second should be equal, fourth should be lowest
        assert!(payouts[0] > payouts[1]);
        assert!(payouts[1] > payouts[3]);
    }
    
    #[test]
    fn test_invalid_prize_pool() {
        let ranked_results = vec![("test".to_string(), 0.5)];
        
        let result = calculate_payouts(&ranked_results, -10.0);
        assert!(matches!(result, Err(crate::error::CliptionsError::Scoring(ScoringError::InvalidPrizePool { .. }))));
        
        let result = calculate_payouts(&ranked_results, 0.0);
        assert!(matches!(result, Err(crate::error::CliptionsError::Scoring(ScoringError::InvalidPrizePool { .. }))));
    }
    
    #[test]
    fn test_empty_guesses() {
        let embedder = MockEmbedder::new(128);
        let strategy = ClipBatchStrategy::new();
        let validator = ScoreValidator::new(embedder, strategy);
        
        let result = calculate_rankings("test.jpg", &[], &validator);
        assert!(matches!(result, Err(crate::error::CliptionsError::Scoring(ScoringError::EmptyGuesses))));
    }
}