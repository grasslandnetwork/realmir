//! Cryptographic commitment system for Cliptions
//! 
//! This module provides secure commitment generation and verification using SHA-256 hashing.
//! The commitment scheme ensures that participants can commit to their guesses without revealing
//! them until the reveal phase, preventing gaming of the system.

use sha2::{Digest, Sha256};
use crate::error::{CommitmentError, Result};



/// Commitment generator for creating cryptographic commitments
#[derive(Debug, Clone)]
pub struct CommitmentGenerator {
    salt_length: usize,
}

impl CommitmentGenerator {
    /// Create a new commitment generator with default salt length
    pub fn new() -> Self {
        Self {
            salt_length: 32,
        }
    }
    
    /// Create a commitment generator with custom salt length
    pub fn with_salt_length(salt_length: usize) -> Self {
        Self { salt_length }
    }
    
    /// Generate a commitment hash from a message and salt
    /// 
    /// This matches the Python implementation: hash(message + salt)
    /// 
    /// # Arguments
    /// * `message` - The plaintext message to commit to
    /// * `salt` - A random salt value to prevent brute force attacks
    /// 
    /// # Returns
    /// The hex-encoded SHA-256 hash of the message concatenated with the salt
    /// 
    /// # Errors
    /// Returns `CommitmentError::EmptySalt` if the salt is empty
    pub fn generate(&self, message: &str, salt: &str) -> Result<String> {
        if message.trim().is_empty() {
            return Err(CommitmentError::EmptyMessage.into());
        }
        if salt.is_empty() {
            return Err(CommitmentError::EmptySalt.into());
        }
        
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        hasher.update(salt.as_bytes());
        let result = hasher.finalize();
        
        Ok(format!("{:x}", result))
    }
    
    /// Generate a random salt of the specified length
    /// 
    /// # Returns
    /// A random hex-encoded salt string
    pub fn generate_salt(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..self.salt_length).map(|_| rng.gen()).collect();
        hex::encode(bytes)
    }
}

impl Default for CommitmentGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Commitment verifier for validating commitments
#[derive(Debug, Clone)]
pub struct CommitmentVerifier {
    generator: CommitmentGenerator,
}

impl CommitmentVerifier {
    /// Create a new commitment verifier
    pub fn new() -> Self {
        Self {
            generator: CommitmentGenerator::new(),
        }
    }
    
    /// Verify that a commitment matches the provided message and salt
    /// 
    /// # Arguments
    /// * `message` - The plaintext message
    /// * `salt` - The salt used in the original commitment
    /// * `commitment` - The commitment hash to verify against
    /// 
    /// # Returns
    /// `true` if the commitment is valid, `false` otherwise
    pub fn verify(&self, message: &str, salt: &str, commitment: &str) -> bool {
        match self.generator.generate(message, salt) {
            Ok(calculated_commitment) => calculated_commitment == commitment,
            Err(_) => false,
        }
    }
    
    /// Batch verify multiple commitments
    /// 
    /// # Arguments
    /// * `commitments` - A slice of tuples containing (message, salt, commitment)
    /// 
    /// # Returns
    /// A vector of boolean values indicating whether each commitment is valid
    pub fn verify_batch(&self, commitments: &[(&str, &str, &str)]) -> Vec<bool> {
        commitments
            .iter()
            .map(|(message, salt, commitment)| self.verify(message, salt, commitment))
            .collect()
    }
    
    /// Parallel batch verification for better performance
    /// 
    /// # Arguments
    /// * `commitments` - A slice of tuples containing (message, salt, commitment)
    /// 
    /// # Returns
    /// A vector of boolean values indicating whether each commitment is valid
    pub fn verify_batch_parallel(&self, commitments: &[(&str, &str, &str)]) -> Vec<bool> {
        use rayon::prelude::*;
        
        commitments
            .par_iter()
            .map(|(message, salt, commitment)| self.verify(message, salt, commitment))
            .collect()
    }
}

impl Default for CommitmentVerifier {
    fn default() -> Self {
        Self::new()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_commitment_generation() {
        let generator = CommitmentGenerator::new();
        let message = "Hello, World!";
        let salt = "random_salt_123";
        
        let commitment = generator.generate(message, salt).unwrap();
        
        // Should be 64 characters (32 bytes in hex)
        assert_eq!(commitment.len(), 64);
        
        // Should be deterministic
        let commitment2 = generator.generate(message, salt).unwrap();
        assert_eq!(commitment, commitment2);
    }
    
    #[test]
    fn test_commitment_verification() {
        let generator = CommitmentGenerator::new();
        let verifier = CommitmentVerifier::new();
        let message = "Test message";
        let salt = "test_salt";
        
        let commitment = generator.generate(message, salt).unwrap();
        
        // Valid commitment should verify
        assert!(verifier.verify(message, salt, &commitment));
        
        // Invalid commitment should not verify
        assert!(!verifier.verify("wrong message", salt, &commitment));
        assert!(!verifier.verify(message, "wrong salt", &commitment));
        assert!(!verifier.verify(message, salt, "wrong_commitment"));
    }
    
    #[test]
    fn test_empty_salt() {
        let generator = CommitmentGenerator::new();
        let result = generator.generate("message", "");
        
        assert!(matches!(result, Err(crate::error::CliptionsError::Commitment(CommitmentError::EmptySalt))));
    }
    
    #[test]
    fn test_empty_message() {
        let generator = CommitmentGenerator::new();
        
        // Test completely empty message
        let result = generator.generate("", "salt");
        assert!(matches!(result, Err(crate::error::CliptionsError::Commitment(CommitmentError::EmptyMessage))));
        
        // Test whitespace-only message
        let result = generator.generate("   ", "salt");
        assert!(matches!(result, Err(crate::error::CliptionsError::Commitment(CommitmentError::EmptyMessage))));
        
        // Test tab and newline only
        let result = generator.generate("\t\n  ", "salt");
        assert!(matches!(result, Err(crate::error::CliptionsError::Commitment(CommitmentError::EmptyMessage))));
    }
    
    #[test]
    fn test_salt_generation() {
        let generator = CommitmentGenerator::new();
        let salt1 = generator.generate_salt();
        let salt2 = generator.generate_salt();
        
        // Salts should be different
        assert_ne!(salt1, salt2);
        
        // Salts should be hex-encoded (64 characters for 32 bytes)
        assert_eq!(salt1.len(), 64);
        assert_eq!(salt2.len(), 64);
        
        // Should be valid hex
        assert!(salt1.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(salt2.chars().all(|c| c.is_ascii_hexdigit()));
    }
    
    #[test]
    fn test_batch_verification() {
        let generator = CommitmentGenerator::new();
        let verifier = CommitmentVerifier::new();
        
        let commitments = vec![
            ("message1", "salt1", generator.generate("message1", "salt1").unwrap()),
            ("message2", "salt2", generator.generate("message2", "salt2").unwrap()),
            ("message3", "salt3", "invalid_commitment".to_string()),
        ];
        
        let commitment_refs: Vec<(&str, &str, &str)> = commitments
            .iter()
            .map(|(m, s, c)| (*m, *s, c.as_str()))
            .collect();
        
        let results = verifier.verify_batch(&commitment_refs);
        
        assert_eq!(results, vec![true, true, false]);
    }
    
    #[test]
    fn test_parallel_batch_verification() {
        let generator = CommitmentGenerator::new();
        let verifier = CommitmentVerifier::new();
        
        // Create a larger batch for parallel testing
        let mut commitments = Vec::new();
        for i in 0..100 {
            let message = format!("message{}", i);
            let salt = format!("salt{}", i);
            let commitment = generator.generate(&message, &salt).unwrap();
            commitments.push((message, salt, commitment));
        }
        
        let commitment_refs: Vec<(&str, &str, &str)> = commitments
            .iter()
            .map(|(m, s, c)| (m.as_str(), s.as_str(), c.as_str()))
            .collect();
        
        let sequential_results = verifier.verify_batch(&commitment_refs);
        let parallel_results = verifier.verify_batch_parallel(&commitment_refs);
        
        // Results should be identical
        assert_eq!(sequential_results, parallel_results);
        
        // All should be valid
        assert!(sequential_results.iter().all(|&r| r));
    }
    
    #[test]
    fn test_rust_core_functionality() {
        // Test that our core implementation works correctly
        let generator = CommitmentGenerator::new();
        let verifier = CommitmentVerifier::new();
        let message = "test message";
        let salt = "test_salt";
        
        let commitment = generator.generate(message, salt).unwrap();
        assert!(verifier.verify(message, salt, &commitment));
        
        // Test with different inputs
        assert!(!verifier.verify("different message", salt, &commitment));
        assert!(!verifier.verify(message, "different_salt", &commitment));
    }
}