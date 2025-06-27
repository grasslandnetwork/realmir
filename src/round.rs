//! Round processing for Cliptions prediction markets
//! 
//! This module handles the complete lifecycle of prediction rounds,
//! including participant management, commitment verification, scoring, and payout calculation.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json;

use crate::commitment::{CommitmentVerifier};
use crate::embedder::{EmbedderTrait};
use crate::scoring::{ScoringStrategy, ScoreValidator, process_participants};
use crate::types::{RoundData, RoundStatus, Participant, ScoringResult, RoundConfig};
use crate::error::{RoundError, Result};

/// Round processor for managing prediction rounds
pub struct RoundProcessor<E: EmbedderTrait, S: ScoringStrategy> {
    rounds_file: String,
    commitment_verifier: CommitmentVerifier,
    score_validator: ScoreValidator<E, S>,
    rounds_cache: HashMap<String, RoundData>,
}

impl<E: EmbedderTrait, S: ScoringStrategy> RoundProcessor<E, S> {
    /// Create a new round processor
    pub fn new(rounds_file: String, embedder: E, scoring_strategy: S) -> Self {
        Self {
            rounds_file,
            commitment_verifier: CommitmentVerifier::new(),
            score_validator: ScoreValidator::new(embedder, scoring_strategy),
            rounds_cache: HashMap::new(),
        }
    }
    
    /// Load rounds data from file
    pub fn load_rounds(&mut self) -> Result<()> {
        if !Path::new(&self.rounds_file).exists() {
            // Create empty rounds file if it doesn't exist
            let empty_rounds: HashMap<String, RoundData> = HashMap::new();
            self.save_rounds(&empty_rounds)?;
            return Ok(());
        }
        
        let content = fs::read_to_string(&self.rounds_file)
            .map_err(|_e| RoundError::DataFileNotFound { 
                path: self.rounds_file.clone() 
            })?;
        
        // Handle empty file case
        if content.trim().is_empty() {
            let empty_rounds: HashMap<String, RoundData> = HashMap::new();
            self.save_rounds(&empty_rounds)?;
            return Ok(());
        }
        
        let rounds: HashMap<String, RoundData> = serde_json::from_str(&content)?;
        self.rounds_cache = rounds;
        
        Ok(())
    }
    
    /// Save rounds data to file
    pub fn save_rounds(&self, rounds: &HashMap<String, RoundData>) -> Result<()> {
        let content = serde_json::to_string_pretty(rounds)?;
        fs::write(&self.rounds_file, content)?;
        Ok(())
    }
    
    /// Get a round by ID
    pub fn get_round(&mut self, round_id: &str) -> Result<&RoundData> {
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        self.rounds_cache.get(round_id)
            .ok_or_else(|| RoundError::RoundNotFound { 
                round_id: round_id.to_string() 
            }.into())
    }
    
    /// Get a mutable reference to a round
    pub fn get_round_mut(&mut self, round_id: &str) -> Result<&mut RoundData> {
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        self.rounds_cache.get_mut(round_id)
            .ok_or_else(|| RoundError::RoundNotFound { 
                round_id: round_id.to_string() 
            }.into())
    }
    
    /// Create a new round
    pub fn create_round(
        &mut self,
        round_id: String,
        title: String,
        description: String,
        target_image_path: String,
        config: Option<RoundConfig>,
    ) -> Result<()> {
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        if self.rounds_cache.contains_key(&round_id) {
            return Err(RoundError::AlreadyProcessed.into());
        }
        
        let mut round = RoundData::new(round_id.clone(), title, description, target_image_path);
        if let Some(config) = config {
            round.config = config;
        }
        
        self.rounds_cache.insert(round_id, round);
        self.save_rounds(&self.rounds_cache)?;
        
        Ok(())
    }
    
    /// Add a participant to a round
    pub fn add_participant(&mut self, round_id: &str, participant: Participant) -> Result<()> {
        let round = self.get_round_mut(round_id)?;
        
        if !round.is_open() {
            return Err(RoundError::AlreadyProcessed.into());
        }
        
        round.add_participant(participant);
        self.save_rounds(&self.rounds_cache)?;
        
        Ok(())
    }
    
    /// Verify commitments for a round
    pub fn verify_commitments(&mut self, round_id: &str) -> Result<Vec<bool>> {
        // Load rounds if needed
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        let round = self.rounds_cache.get_mut(round_id)
            .ok_or_else(|| RoundError::RoundNotFound { 
                round_id: round_id.to_string() 
            })?;
        
        let mut results = Vec::new();
        
        for participant in &mut round.participants {
            if let Some(salt) = &participant.salt {
                let is_valid = self.commitment_verifier.verify(
                    &participant.guess.text,
                    salt,
                    &participant.commitment,
                );
                
                if is_valid {
                    participant.verified = true;
                }
                
                results.push(is_valid);
            } else {
                results.push(false);
            }
        }
        
        self.save_rounds(&self.rounds_cache)?;
        Ok(results)
    }
    
    /// Process round payouts
    pub fn process_round_payouts(&mut self, round_id: &str) -> Result<Vec<ScoringResult>> {
        // Load rounds if needed
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        // Get round data first
        let (target_image_path, prize_pool, verified_participants) = {
            let round = self.rounds_cache.get(round_id)
                .ok_or_else(|| RoundError::RoundNotFound { 
                    round_id: round_id.to_string() 
                })?;
            
            // Verify target image exists
            if !Path::new(&round.target_image_path).exists() {
                return Err(RoundError::TargetImageNotFound { 
                    path: round.target_image_path.clone() 
                }.into());
            }
            
            // Get verified participants
            let verified_participants: Vec<Participant> = round.participants
                .iter()
                .filter(|p| p.verified)
                .cloned()
                .collect();
            
            if verified_participants.is_empty() {
                return Err(RoundError::NoParticipants { 
                    round_id: round_id.to_string() 
                }.into());
            }
            
            (round.target_image_path.clone(), round.config.prize_pool, verified_participants)
        };
        
        // Process participants and calculate scores
        let results = process_participants(
            &verified_participants,
            &target_image_path,
            prize_pool,
            &self.score_validator,
        )?;
        
        // Update round with results
        let round = self.rounds_cache.get_mut(round_id).unwrap(); // Safe because we checked above
        round.set_results(results.clone());
        self.save_rounds(&self.rounds_cache)?;
        
        Ok(results)
    }
    
    /// Get all round IDs
    pub fn get_round_ids(&mut self) -> Result<Vec<String>> {
        if self.rounds_cache.is_empty() {
            self.load_rounds()?;
        }
        
        Ok(self.rounds_cache.keys().cloned().collect())
    }
    
    /// Process all rounds
    pub fn process_all_rounds(&mut self) -> Result<HashMap<String, Vec<ScoringResult>>> {
        let round_ids = self.get_round_ids()?;
        let mut all_results = HashMap::new();
        
        for round_id in round_ids {
            // Only process rounds that are open or processing
            let round = self.get_round(&round_id)?;
            if matches!(round.status, RoundStatus::Open | RoundStatus::Processing) {
                match self.process_round_payouts(&round_id) {
                    Ok(results) => {
                        all_results.insert(round_id, results);
                    }
                    Err(e) => {
                        eprintln!("Failed to process round {}: {}", round_id, e);
                    }
                }
            }
        }
        
        Ok(all_results)
    }
    
    /// Get round statistics
    pub fn get_round_stats(&mut self, round_id: &str) -> Result<RoundStats> {
        let round = self.get_round(round_id)?;
        
        let total_participants = round.participants.len();
        let verified_participants = round.verified_participants().len();
        let total_prize_pool = round.config.prize_pool;
        let is_complete = round.is_complete();
        
        let total_payout = if is_complete {
            round.results.iter()
                .filter_map(|r| r.payout)
                .sum()
        } else {
            0.0
        };
        
        Ok(RoundStats {
            round_id: round_id.to_string(),
            total_participants,
            verified_participants,
            total_prize_pool,
            total_payout,
            is_complete,
            status: round.status.clone(),
        })
    }
}

/// Statistics for a round
#[derive(Debug, Clone)]
pub struct RoundStats {
    pub round_id: String,
    pub total_participants: usize,
    pub verified_participants: usize,
    pub total_prize_pool: f64,
    pub total_payout: f64,
    pub is_complete: bool,
    pub status: RoundStatus,
}



#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::types::Guess;
    use crate::embedder::MockEmbedder;
    use crate::scoring::ClipBatchStrategy;
    
    fn create_test_processor() -> (RoundProcessor<MockEmbedder, ClipBatchStrategy>, String) {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_string_lossy().to_string();
        
        let embedder = MockEmbedder::clip_like();
        let strategy = ClipBatchStrategy::new();
        let processor = RoundProcessor::new(file_path.clone(), embedder, strategy);
        
        (processor, file_path)
    }
    
    fn create_test_participant(user_id: &str, guess_text: &str, commitment: &str) -> Participant {
        let guess = Guess::new(guess_text.to_string());
        Participant::new(
            user_id.to_string(),
            format!("user_{}", user_id),
            guess,
            commitment.to_string(),
        ).with_salt("test_salt".to_string()).mark_verified()
    }
    
    #[test]
    fn test_round_processor_creation() {
        let (mut processor, _) = create_test_processor();
        
        // Should be able to load empty rounds
        processor.load_rounds().unwrap();
        assert!(processor.get_round_ids().unwrap().is_empty());
    }
    
    #[test]
    fn test_create_round() {
        let (mut processor, _) = create_test_processor();
        
        processor.create_round(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
            None,
        ).unwrap();
        
        let round = processor.get_round("test_round").unwrap();
        assert_eq!(round.round_id, "test_round");
        assert_eq!(round.title, "Test Round");
        assert!(round.is_open());
    }
    
    #[test]
    fn test_add_participant() {
        let (mut processor, _) = create_test_processor();
        
        processor.create_round(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
            None,
        ).unwrap();
        
        let participant = create_test_participant("user1", "test guess", "commitment123");
        
        processor.add_participant("test_round", participant).unwrap();
        
        let round = processor.get_round("test_round").unwrap();
        assert_eq!(round.participants.len(), 1);
        assert_eq!(round.participants[0].user_id, "user1");
    }
    
    #[test]
    fn test_verify_commitments() {
        let (mut processor, _) = create_test_processor();
        
        processor.create_round(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
            None,
        ).unwrap();
        
        // Create a valid commitment
        let commitment_gen = crate::commitment::CommitmentGenerator::new();
        let salt = "test_salt";
        let message = "test guess";
        let commitment = commitment_gen.generate(message, salt).unwrap();
        
        let participant = Participant::new(
            "user1".to_string(),
            "user_user1".to_string(),
            Guess::new(message.to_string()),
            commitment,
        ).with_salt(salt.to_string());
        
        processor.add_participant("test_round", participant).unwrap();
        
        let results = processor.verify_commitments("test_round").unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]); // Should be valid
        
        let round = processor.get_round("test_round").unwrap();
        assert!(round.participants[0].verified);
    }
    
    #[test]
    fn test_round_stats() {
        let (mut processor, _) = create_test_processor();
        
        processor.create_round(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
            None,
        ).unwrap();
        
        let participant = create_test_participant("user1", "test guess", "commitment123");
        processor.add_participant("test_round", participant).unwrap();
        
        let stats = processor.get_round_stats("test_round").unwrap();
        assert_eq!(stats.round_id, "test_round");
        assert_eq!(stats.total_participants, 1);
        assert_eq!(stats.verified_participants, 1);
        assert!(!stats.is_complete);
    }
    
    #[test]
    fn test_nonexistent_round() {
        let (mut processor, _) = create_test_processor();
        
        let result = processor.get_round("nonexistent");
        assert!(matches!(result, Err(crate::error::CliptionsError::Round(RoundError::RoundNotFound { .. }))));
    }
}