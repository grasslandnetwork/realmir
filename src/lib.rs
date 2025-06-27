//! # Cliptions Core
//!
//! High-performance Rust implementation of the Cliptions prediction market core functionality.
//! This library provides cryptographic commitments, scoring strategies, embedding integration,
//! and round processing capabilities.
//! 
//! ## Features
//! 
//! - **Commitment System**: Secure commitment generation and verification using SHA-256
//! - **Scoring Strategies**: Multiple scoring algorithms including CLIP batch processing
//! - **Embedding Integration**: Interface for CLIP and other embedding models
//! - **Round Processing**: Complete round lifecycle management
//! - **Pure Rust Core**: Clean separation between core logic and language bindings
//! 
//! ## Architecture
//! 
//! The library follows SOLID principles and uses the Strategy pattern for scoring algorithms.
//! Core traits define interfaces for embedding models and scoring strategies, allowing
//! for easy extension and testing.

// Core library modules
pub mod commitment;
pub mod config;
pub mod embedder;
pub mod error;
pub mod models;
pub mod payout;
pub mod round;
pub mod scoring;
pub mod social;
pub mod types;

// Python bindings module (conditional compilation)
#[cfg(feature = "python")]
pub mod python_bridge;

// Re-export commonly used types
pub use commitment::{CommitmentGenerator, CommitmentVerifier};
pub use scoring::{ScoringStrategy, ClipBatchStrategy, ScoreValidator};
pub use embedder::{EmbedderTrait, MockEmbedder};
pub use round::{RoundProcessor};
pub use payout::{PayoutCalculator, PayoutConfig, PayoutInfo};
pub use config::{ConfigManager, CostTracker, CliptionsConfig, OpenAIConfig, SpendingStatus};
pub use social::{SocialWorkflow, AnnouncementFormatter, UrlParser, HashtagManager, AnnouncementData, TweetId};
pub use types::{Guess, Participant, ScoringResult, RoundData};
pub use error::{CliptionsError, Result};

// Re-export Python module when feature is enabled
#[cfg(feature = "python")]
pub use python_bridge::*;