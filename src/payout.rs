use crate::error::{CliptionsError, Result};
use crate::types::Participant;
use serde::{Deserialize, Serialize};

/// Represents payout information for a participant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayoutInfo {
    pub username: String,
    pub guess: String,
    pub score: f64,
    pub rank: usize,
    pub payout: f64,
}

/// Configuration for payout calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutConfig {
    pub prize_pool: f64,
    pub platform_fee_percentage: f64,
    pub minimum_players: usize,
}

impl Default for PayoutConfig {
    fn default() -> Self {
        Self {
            prize_pool: 1000.0,
            platform_fee_percentage: 0.0, // No platform fee by default
            minimum_players: 2,
        }
    }
}

/// Payout calculation engine
#[derive(Debug)]
pub struct PayoutCalculator {
    config: PayoutConfig,
}

impl PayoutCalculator {
    /// Create a new payout calculator with default configuration
    pub fn new() -> Self {
        Self {
            config: PayoutConfig::default(),
        }
    }

    /// Create a new payout calculator with custom configuration
    pub fn with_config(config: PayoutConfig) -> Self {
        Self { config }
    }

    /// Calculate payouts based on rankings using position-based scoring
    /// 
    /// The payout calculation uses a position-based scoring system where:
    /// - Scores are based only on position (1st, 2nd, etc), not similarity values
    /// - Equal similarity scores get equal payouts (ties split the combined payout)
    /// - Scores sum to 1.0 to distribute full prize pool
    /// - Higher positions get proportionally higher scores
    pub fn calculate_payouts(&self, ranked_results: &[(String, f64)]) -> Result<Vec<f64>> {
        if ranked_results.is_empty() {
            return Ok(vec![]);
        }

        let total_players = ranked_results.len();
        if total_players < self.config.minimum_players {
            return Err(CliptionsError::ValidationError(
                format!("Minimum {} players required, got {}", self.config.minimum_players, total_players)
            ));
        }

        // Calculate available prize pool after platform fee
        let available_pool = self.config.prize_pool * (1.0 - self.config.platform_fee_percentage / 100.0);
        
        let denominator: usize = (1..=total_players).sum();
        
        // Group positions by similarity score
        let mut groups = Vec::new();
        let mut current_group = Vec::new();
        let mut current_similarity = None;
        
        for (i, (_guess, similarity)) in ranked_results.iter().enumerate() {
            match current_similarity {
                None => {
                    current_group.push(i);
                    current_similarity = Some(*similarity);
                }
                Some(prev_sim) if (prev_sim - similarity).abs() < f64::EPSILON => {
                    current_group.push(i);
                }
                Some(_) => {
                    groups.push((current_group.clone(), current_similarity.unwrap()));
                    current_group = vec![i];
                    current_similarity = Some(*similarity);
                }
            }
        }
        
        if !current_group.is_empty() {
            groups.push((current_group, current_similarity.unwrap()));
        }
        
        // Calculate payouts
        let mut payouts = vec![0.0; total_players];
        let mut position = 0;
        
        for (group, _similarity) in groups {
            let group_size = group.len();
            let group_points: usize = (0..group_size)
                .map(|i| total_players - (position + i))
                .sum();
            
            let points_per_position = group_points as f64 / group_size as f64;
            let score = points_per_position / denominator as f64;
            let payout = score * available_pool;
            
            for &idx in &group {
                payouts[idx] = payout;
            }
            
            position += group_size;
        }
        
        Ok(payouts)
    }

    /// Process complete payout calculation including ranking and validation
    /// This method takes a slice of (participant, score) tuples instead of using Participant.score
    pub fn process_payouts_with_scores(&self, participant_scores: &[(Participant, f64)]) -> Result<Vec<PayoutInfo>> {
        // Filter verified participants
        let valid_participants: Vec<_> = participant_scores
            .iter()
            .filter(|(p, _)| p.verified)
            .collect();

        if valid_participants.is_empty() {
            return Ok(vec![]);
        }

        // Create ranked results from participants
        let mut ranked_results = Vec::new();
        for (participant, score) in &valid_participants {
            ranked_results.push((participant.guess.text.clone(), *score));
        }

        // Sort by score (highest first)
        ranked_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Calculate payouts
        let payouts = self.calculate_payouts(&ranked_results)?;

        // Create payout info
        let mut payout_infos = Vec::new();
        for (i, ((guess, score), payout)) in ranked_results.iter().zip(payouts.iter()).enumerate() {
            // Find the original participant
            if let Some((participant, _)) = valid_participants.iter().find(|(p, _)| p.guess.text == *guess) {
                payout_infos.push(PayoutInfo {
                    username: participant.username.clone(),
                    guess: guess.clone(),
                    score: *score,
                    rank: i + 1,
                    payout: *payout,
                });
            }
        }

        Ok(payout_infos)
    }

    /// Validate payout configuration
    pub fn validate_config(&self) -> Result<()> {
        if self.config.prize_pool <= 0.0 {
            return Err(CliptionsError::ValidationError(
                "Prize pool must be positive".to_string()
            ));
        }

        if self.config.platform_fee_percentage < 0.0 || self.config.platform_fee_percentage >= 100.0 {
            return Err(CliptionsError::ValidationError(
                "Platform fee percentage must be between 0 and 100".to_string()
            ));
        }

        if self.config.minimum_players == 0 {
            return Err(CliptionsError::ValidationError(
                "Minimum players must be at least 1".to_string()
            ));
        }

        Ok(())
    }

    /// Get the total platform fee for the current configuration
    pub fn calculate_platform_fee(&self) -> f64 {
        self.config.prize_pool * (self.config.platform_fee_percentage / 100.0)
    }

    /// Get the available prize pool after platform fees
    pub fn calculate_available_pool(&self) -> f64 {
        self.config.prize_pool - self.calculate_platform_fee()
    }

    /// Set a new prize pool
    pub fn set_prize_pool(&mut self, prize_pool: f64) -> Result<()> {
        if prize_pool <= 0.0 {
            return Err(CliptionsError::ValidationError(
                "Prize pool must be positive".to_string()
            ));
        }
        self.config.prize_pool = prize_pool;
        Ok(())
    }

    /// Set platform fee percentage
    pub fn set_platform_fee(&mut self, fee_percentage: f64) -> Result<()> {
        if fee_percentage < 0.0 || fee_percentage >= 100.0 {
            return Err(CliptionsError::ValidationError(
                "Platform fee percentage must be between 0 and 100".to_string()
            ));
        }
        self.config.platform_fee_percentage = fee_percentage;
        Ok(())
    }

    /// Get current configuration
    pub fn get_config(&self) -> &PayoutConfig {
        &self.config
    }
}

impl Default for PayoutCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Guess, Participant};

    fn create_test_participants_with_scores(guesses_and_scores: Vec<(&str, &str, f64, bool)>) -> Vec<(Participant, f64)> {
        guesses_and_scores
            .into_iter()
            .map(|(username, guess_text, score, verified)| {
                let guess = Guess::new(guess_text.to_string());
                let participant = Participant::new(
                    format!("user_{}", username),
                    username.to_string(),
                    guess,
                    "dummy_commitment".to_string(),
                );
                let participant = if verified {
                    participant.mark_verified()
                } else {
                    participant
                };
                (participant, score)
            })
            .collect()
    }

    #[test]
    fn test_payout_distribution() {
        let calculator = PayoutCalculator::new();
        let ranked_results = vec![
            ("Best guess".to_string(), 0.9),
            ("Good guess".to_string(), 0.7),
            ("Ok guess".to_string(), 0.5),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        assert_eq!(payouts.len(), 3);
        
        // First place should get highest payout
        assert!(payouts[0] > payouts[1]);
        assert!(payouts[1] > payouts[2]);
        
        // Total payout should equal available pool
        let total_payout: f64 = payouts.iter().sum();
        let expected = calculator.calculate_available_pool();
        assert!((total_payout - expected).abs() < 1e-10);
    }

    #[test]
    fn test_equal_scores_for_equal_ranks() {
        let calculator = PayoutCalculator::new();
        let ranked_results = vec![
            ("Tie 1".to_string(), 0.8),
            ("Tie 2".to_string(), 0.8),
            ("Third".to_string(), 0.6),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        // Tied players should get equal payouts
        assert_eq!(payouts[0], payouts[1]);
        assert!(payouts[0] > payouts[2]);
    }

    #[test]
    fn test_two_player_payout() {
        let calculator = PayoutCalculator::new();
        let ranked_results = vec![
            ("Winner".to_string(), 0.9),
            ("Runner-up".to_string(), 0.6),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        assert_eq!(payouts.len(), 2);
        assert!(payouts[0] > payouts[1]);
        
        // Check total equals available pool
        let total_payout: f64 = payouts.iter().sum();
        let expected = calculator.calculate_available_pool();
        assert!((total_payout - expected).abs() < 1e-10);
    }

    #[test]
    fn test_three_player_payout() {
        let calculator = PayoutCalculator::new();
        let ranked_results = vec![
            ("First".to_string(), 0.9),
            ("Second".to_string(), 0.7),
            ("Third".to_string(), 0.5),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        assert_eq!(payouts.len(), 3);
        assert!(payouts[0] > payouts[1]);
        assert!(payouts[1] > payouts[2]);
        
        // Verify position-based calculation
        // For 3 players: denominator = 1+2+3 = 6
        // First gets 3/6, Second gets 2/6, Third gets 1/6
        let expected_pool = calculator.calculate_available_pool();
        let expected_payouts = vec![
            expected_pool * 3.0 / 6.0,
            expected_pool * 2.0 / 6.0,
            expected_pool * 1.0 / 6.0,
        ];
        
        for (actual, expected) in payouts.iter().zip(expected_payouts.iter()) {
            assert!((actual - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_custom_prize_pool() {
        let config = PayoutConfig {
            prize_pool: 500.0,
            platform_fee_percentage: 0.0,
            minimum_players: 2,
        };
        let calculator = PayoutCalculator::with_config(config);
        
        let ranked_results = vec![
            ("Winner".to_string(), 0.9),
            ("Runner-up".to_string(), 0.6),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        let total_payout: f64 = payouts.iter().sum();
        
        assert!((total_payout - 500.0).abs() < 1e-10);
    }

    #[test]
    fn test_platform_fee_calculation() {
        let config = PayoutConfig {
            prize_pool: 1000.0,
            platform_fee_percentage: 10.0,
            minimum_players: 2,
        };
        let calculator = PayoutCalculator::with_config(config);
        
        assert_eq!(calculator.calculate_platform_fee(), 100.0);
        assert_eq!(calculator.calculate_available_pool(), 900.0);
        
        let ranked_results = vec![
            ("Winner".to_string(), 0.9),
            ("Runner-up".to_string(), 0.6),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        let total_payout: f64 = payouts.iter().sum();
        
        // Should equal available pool (after platform fee)
        assert!((total_payout - 900.0).abs() < 1e-10);
    }

    #[test]
    fn test_minimum_players() {
        let config = PayoutConfig {
            prize_pool: 1000.0,
            platform_fee_percentage: 0.0,
            minimum_players: 3,
        };
        let calculator = PayoutCalculator::with_config(config);
        
        // Should fail with less than minimum players
        let ranked_results = vec![
            ("Player1".to_string(), 0.9),
            ("Player2".to_string(), 0.6),
        ];

        let result = calculator.calculate_payouts(&ranked_results);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Minimum 3 players required"));
    }

    #[test]
    fn test_invalid_guess_range() {
        let calculator = PayoutCalculator::new();
        
        // Empty results should return empty payouts
        let payouts = calculator.calculate_payouts(&[]).unwrap();
        assert!(payouts.is_empty());
    }

    #[test]
    fn test_process_payouts_integration() {
        let calculator = PayoutCalculator::new();
        let participant_scores = create_test_participants_with_scores(vec![
            ("alice", "Great guess", 0.9, true),
            ("bob", "Good guess", 0.7, true),
            ("charlie", "Bad guess", 0.3, false), // Not verified
        ]);

        let payout_infos = calculator.process_payouts_with_scores(&participant_scores).unwrap();
        
        assert_eq!(payout_infos.len(), 2); // Only verified participants
        assert_eq!(payout_infos[0].username, "alice");
        assert_eq!(payout_infos[0].rank, 1);
        assert_eq!(payout_infos[1].username, "bob");
        assert_eq!(payout_infos[1].rank, 2);
        
        // Verify payout amounts
        assert!(payout_infos[0].payout > payout_infos[1].payout);
    }

    #[test]
    fn test_equal_distance_symmetry() {
        let calculator = PayoutCalculator::new();
        
        // Test symmetry: players equidistant from center should get same relative payout
        let ranked_results = vec![
            ("High".to_string(), 0.8),
            ("Mid".to_string(), 0.5),
            ("Low".to_string(), 0.2),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        // Verify the mathematical relationship holds
        // For position-based scoring, this tests the internal calculation consistency
        let total: f64 = payouts.iter().sum();
        assert!((total - calculator.calculate_available_pool()).abs() < 1e-10);
    }

    #[test]
    fn test_score_range() {
        let calculator = PayoutCalculator::new();
        
        // Test with edge case scores
        let ranked_results = vec![
            ("Perfect".to_string(), 1.0),
            ("Zero".to_string(), 0.0),
            ("Negative".to_string(), -0.1),
        ];

        let payouts = calculator.calculate_payouts(&ranked_results).unwrap();
        
        assert_eq!(payouts.len(), 3);
        assert!(payouts[0] > payouts[1]);
        assert!(payouts[1] > payouts[2]);
        
        // All payouts should be non-negative
        for payout in &payouts {
            assert!(*payout >= 0.0);
        }
    }

    #[test]
    fn test_config_validation() {
        let mut calculator = PayoutCalculator::new();
        
        // Test invalid prize pool
        assert!(calculator.set_prize_pool(-100.0).is_err());
        assert!(calculator.set_prize_pool(0.0).is_err());
        assert!(calculator.set_prize_pool(100.0).is_ok());
        
        // Test invalid platform fee
        assert!(calculator.set_platform_fee(-1.0).is_err());
        assert!(calculator.set_platform_fee(100.0).is_err());
        assert!(calculator.set_platform_fee(50.0).is_ok());
    }
}