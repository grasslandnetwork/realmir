//! Core data types for Cliptions
//!
//! This module defines the fundamental data structures used throughout the Cliptions system,
//! including participants, guesses, scoring results, and round data.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use ndarray::Array1;
use std::collections::HashMap;

/// A participant's guess in the prediction market
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Guess {
    /// The text content of the guess
    pub text: String,
    /// Embedding vector for the guess (if computed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f64>>,
    /// Timestamp when the guess was made
    pub timestamp: DateTime<Utc>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl Guess {
    /// Create a new guess with the current timestamp
    pub fn new(text: String) -> Self {
        Self {
            text,
            embedding: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }
    
    /// Create a guess with a specific timestamp
    pub fn with_timestamp(text: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            text,
            embedding: None,
            timestamp,
            metadata: HashMap::new(),
        }
    }
    
    /// Set the embedding for this guess
    pub fn with_embedding(mut self, embedding: Vec<f64>) -> Self {
        self.embedding = Some(embedding);
        self
    }
    
    /// Add metadata to the guess
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
    
    /// Get the embedding as an ndarray
    pub fn get_embedding_array(&self) -> Option<Array1<f64>> {
        self.embedding.as_ref().map(|e| Array1::from_vec(e.clone()))
    }
}

/// A participant in the prediction market
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Participant {
    /// Unique identifier for the participant
    pub user_id: String,
    /// Display name or username
    pub username: String,
    /// The participant's guess
    pub guess: Guess,
    /// Cryptographic commitment to the guess
    pub commitment: String,
    /// Salt used for the commitment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salt: Option<String>,
    /// Whether the commitment has been verified
    #[serde(default)]
    pub verified: bool,
}

impl Participant {
    /// Create a new participant
    pub fn new(user_id: String, username: String, guess: Guess, commitment: String) -> Self {
        Self {
            user_id,
            username,
            guess,
            commitment,
            salt: None,
            verified: false,
        }
    }
    
    /// Set the salt for commitment verification
    pub fn with_salt(mut self, salt: String) -> Self {
        self.salt = Some(salt);
        self
    }
    
    /// Mark the participant as verified
    pub fn mark_verified(mut self) -> Self {
        self.verified = true;
        self
    }
}

/// Result of scoring a participant's guess
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoringResult {
    /// Reference to the participant
    pub participant: Participant,
    /// Raw similarity score
    pub raw_score: f64,
    /// Adjusted similarity score (if applicable)
    pub adjusted_score: Option<f64>,
    /// Final rank in the competition
    pub rank: Option<usize>,
    /// Calculated payout amount
    pub payout: Option<f64>,
}

impl ScoringResult {
    /// Create a new scoring result
    pub fn new(participant: Participant, raw_score: f64) -> Self {
        Self {
            participant,
            raw_score,
            adjusted_score: None,
            rank: None,
            payout: None,
        }
    }
    
    /// Set the adjusted score
    pub fn with_adjusted_score(mut self, adjusted_score: f64) -> Self {
        self.adjusted_score = Some(adjusted_score);
        self
    }
    
    /// Set the rank
    pub fn with_rank(mut self, rank: usize) -> Self {
        self.rank = Some(rank);
        self
    }
    
    /// Set the payout
    pub fn with_payout(mut self, payout: f64) -> Self {
        self.payout = Some(payout);
        self
    }
    
    /// Get the effective score (adjusted if available, otherwise raw)
    pub fn effective_score(&self) -> f64 {
        self.adjusted_score.unwrap_or(self.raw_score)
    }
}

/// Configuration for a prediction round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundConfig {
    /// Prize pool for the round
    pub prize_pool: f64,
    /// Maximum length for guesses
    pub max_guess_length: usize,
    /// Scoring version to use for this round
    pub scoring_version: String,
}

impl Default for RoundConfig {
    fn default() -> Self {
        Self {
            prize_pool: 100.0,
            max_guess_length: 300,
            scoring_version: "v0.3".to_string(),
        }
    }
}

/// Status of a prediction round
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RoundStatus {
    /// Round is accepting submissions
    Open,
    /// Round is closed, processing results
    Processing,
    /// Round is complete with results
    Complete,
    /// Round was cancelled
    Cancelled,
}

/// Complete data for a prediction round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundData {
    /// Unique identifier for the round
    pub round_id: String,
    /// Human-readable title
    pub title: String,
    /// Description of the round
    pub description: String,
    /// Path to the target image
    pub target_image_path: String,
    /// Current status of the round
    pub status: RoundStatus,
    /// Configuration for the round
    pub config: RoundConfig,
    /// List of participants
    pub participants: Vec<Participant>,
    /// Scoring results (if processed)
    #[serde(default)]
    pub results: Vec<ScoringResult>,
    /// Timestamp when the round was created
    pub created_at: DateTime<Utc>,
    /// Timestamp when the round was last updated
    pub updated_at: DateTime<Utc>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl RoundData {
    /// Create a new round
    pub fn new(
        round_id: String,
        title: String,
        description: String,
        target_image_path: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            round_id,
            title,
            description,
            target_image_path,
            status: RoundStatus::Open,
            config: RoundConfig::default(),
            participants: Vec::new(),
            results: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }
    
    /// Add a participant to the round
    pub fn add_participant(&mut self, participant: Participant) {
        self.participants.push(participant);
        self.updated_at = Utc::now();
    }
    
    /// Update the round status
    pub fn set_status(&mut self, status: RoundStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
    
    /// Set the results for the round
    pub fn set_results(&mut self, results: Vec<ScoringResult>) {
        self.results = results;
        self.status = RoundStatus::Complete;
        self.updated_at = Utc::now();
    }
    
    /// Get participants with verified commitments
    pub fn verified_participants(&self) -> Vec<&Participant> {
        self.participants.iter().filter(|p| p.verified).collect()
    }
    
    /// Check if the round is open for submissions
    pub fn is_open(&self) -> bool {
        matches!(self.status, RoundStatus::Open)
    }
    
    /// Check if the round is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, RoundStatus::Complete)
    }
}

/// Payout result for a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutResult {
    /// Participant information
    pub participant: Participant,
    /// Amount to be paid out
    pub amount: f64,
    /// Rank in the competition
    pub rank: usize,
    /// Score that determined the rank
    pub score: f64,
}

impl PayoutResult {
    /// Create a new payout result
    pub fn new(participant: Participant, amount: f64, rank: usize, score: f64) -> Self {
        Self {
            participant,
            amount,
            rank,
            score,
        }
    }
}