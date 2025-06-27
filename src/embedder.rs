//! Embedding model interface for Cliptions
//! 
//! This module provides the interface for embedding models like CLIP,
//! along with a mock implementation for testing and development.

use ndarray::Array1;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use crate::error::{EmbeddingError, Result};

// Candle imports for native CLIP support
use candle_core::{Device, Tensor, DType};
use candle_nn::{VarBuilder, ops::softmax};
use candle_transformers::models::clip::{ClipModel, ClipConfig};
use tokenizers::Tokenizer;
use std::fs;

/// Trait for embedding models that can convert images and text to feature vectors
/// 
/// This corresponds to the Python IEmbedder interface
pub trait EmbedderTrait: Send + Sync {
    /// Generate embedding for an image
    /// 
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// 
    /// # Returns
    /// Normalized image embedding vector
    fn get_image_embedding(&self, image_path: &str) -> Result<Array1<f64>>;
    
    /// Generate embedding for text
    /// 
    /// # Arguments
    /// * `text` - Text string to embed
    /// 
    /// # Returns
    /// Normalized text embedding vector
    fn get_text_embedding(&self, text: &str) -> Result<Array1<f64>>;
    
    /// Calculate batch similarities between an image and multiple texts using proper CLIP forward pass
    /// 
    /// This is the correct way to rank multiple texts against an image, as it uses
    /// the model's forward pass with softmax to create competitive rankings.
    /// 
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// * `texts` - List of text strings to compare against the image
    /// 
    /// # Returns
    /// Vector of similarity scores (as percentages 0-100) in the same order as input texts
    fn calculate_batch_similarities(&self, image_path: &str, texts: &[String]) -> Result<Vec<f64>>;
    
    /// Get the dimensionality of embeddings produced by this model
    fn embedding_dim(&self) -> usize;
}

/// Mock embedder for testing and development
/// 
/// This embedder generates deterministic embeddings based on hash functions,
/// allowing for consistent testing without requiring actual CLIP models.
#[derive(Debug, Clone)]
pub struct MockEmbedder {
    embedding_dim: usize,
}

impl MockEmbedder {
    /// Create a new mock embedder with specified dimensions
    pub fn new(embedding_dim: usize) -> Self {
        Self { embedding_dim }
    }
    
    /// Create a mock embedder with CLIP-like dimensions (512)
    pub fn clip_like() -> Self {
        Self::new(512)
    }
    
    /// Generate a deterministic embedding from a hash
    fn hash_to_embedding(&self, input: &str) -> Array1<f64> {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Generate deterministic values from the hash
        let mut values = Vec::with_capacity(self.embedding_dim);
        let mut seed = hash;
        
        for _ in 0..self.embedding_dim {
            // Simple linear congruential generator for deterministic values
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let normalized = (seed as f64) / (u64::MAX as f64) * 2.0 - 1.0;
            values.push(normalized);
        }
        
        let mut embedding = Array1::from_vec(values);
        
        // Normalize the embedding vector
        let norm = embedding.dot(&embedding).sqrt();
        if norm > 0.0 {
            embedding /= norm;
        }
        
        embedding
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::clip_like()
    }
}

impl EmbedderTrait for MockEmbedder {
    fn get_image_embedding(&self, image_path: &str) -> Result<Array1<f64>> {
        // For mock purposes, just hash the image path
        // In a real implementation, this would load and process the image
        let embedding_input = format!("image:{}", image_path);
        Ok(self.hash_to_embedding(&embedding_input))
    }
    
    fn get_text_embedding(&self, text: &str) -> Result<Array1<f64>> {
        // Hash the text to create a deterministic embedding
        let embedding_input = format!("text:{}", text);
        Ok(self.hash_to_embedding(&embedding_input))
    }
    
    fn calculate_batch_similarities(&self, image_path: &str, texts: &[String]) -> Result<Vec<f64>> {
        // Get image embedding
        let image_embedding = self.get_image_embedding(image_path)?;
        
        // Calculate raw cosine similarities for all texts
        let mut raw_similarities = Vec::new();
        for text in texts {
            let text_embedding = self.get_text_embedding(text)?;
            let similarity = cosine_similarity(&image_embedding, &text_embedding)?;
            raw_similarities.push(similarity);
        }
        
        // Apply softmax to create competitive rankings (simulating CLIP's behavior)
        let max_sim = raw_similarities.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let exp_sims: Vec<f64> = raw_similarities.iter().map(|&x| (x - max_sim).exp()).collect();
        let sum_exp: f64 = exp_sims.iter().sum();
        
        // Convert to percentages
        let softmax_percentages: Vec<f64> = exp_sims.iter()
            .map(|&x| (x / sum_exp) * 100.0)
            .collect();
        
        Ok(softmax_percentages)
    }
    
    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
}

/// Native Rust CLIP embedder using Candle ML framework
/// 
/// This provides a pure Rust implementation of CLIP using HuggingFace's Candle framework
#[derive(Debug)]
pub struct ClipEmbedder {
    model: ClipModel,
    tokenizer: Tokenizer,
    device: Device,
    embedding_dim: usize,
}

impl ClipEmbedder {
    /// Create a new CLIP embedder with default model (ViT-B/32)
    /// Automatically downloads model files if not found locally
    pub fn new() -> Result<Self> {
        // Try to find local CLIP model files in common locations
        let possible_paths = [
            "models/clip-vit-base-patch32",
            "clip-vit-base-patch32", 
            "models/openai-clip-vit-base-patch32",
            "openai-clip-vit-base-patch32"
        ];
        
        for path in &possible_paths {
            if Self::check_model_files_exist(path) {
                return Self::from_path(path);
            }
        }
        
        // No local models found, try to download
        let download_path = "models/clip-vit-base-patch32";
        println!("CLIP ViT-B/32 model not found locally. Downloading to {}...", download_path);
        
        if let Err(e) = Self::download_model(download_path) {
            eprintln!("Failed to download model: {}", e);
            return Err(EmbeddingError::ModelLoadFailed.into());
        }
        
        println!("Model downloaded successfully!");
        Self::from_path(download_path)
    }
    
    /// Check if all required model files exist in the given path
    fn check_model_files_exist(path: &str) -> bool {
        let required_files = [
            "tokenizer.json",
        ];
        
        // Check if either safetensors or pytorch model exists
        let has_weights = Path::new(path).join("model.safetensors").exists() ||
                         Path::new(path).join("pytorch_model.bin").exists();
        
        if !has_weights {
            return false;
        }
        
        // Check other required files
        for file in &required_files {
            if !Path::new(path).join(file).exists() {
                return false;
            }
        }
        
        true
    }
    
    /// Download CLIP model files from Hugging Face using HF Hub API
    fn download_model(download_path: &str) -> Result<()> {
        // Create directory if it doesn't exist
        fs::create_dir_all(download_path)
            .map_err(|_| EmbeddingError::ModelLoadFailed)?;
        
        // Use HF Hub API like the working version
        let api = hf_hub::api::sync::Api::new()
            .map_err(|_| EmbeddingError::ModelLoadFailed)?;
        let api = api.repo(hf_hub::Repo::with_revision(
            "openai/clip-vit-base-patch32".to_string(),
            hf_hub::RepoType::Model,
            "refs/pr/15".to_string(),
        ));
        
        // Files to download
        let files_to_download = ["tokenizer.json", "model.safetensors"];
        
        for filename in &files_to_download {
            println!("Downloading {}...", filename);
            let file_path = api.get(filename)
                .map_err(|_| EmbeddingError::ModelLoadFailed)?;
            
            let target_path = Path::new(download_path).join(filename);
            fs::copy(&file_path, &target_path)
                .map_err(|_| EmbeddingError::ModelLoadFailed)?;
            
            println!("Downloaded {} to {}", filename, target_path.display());
        }
        
        Ok(())
    }
    
    /// Create CLIP embedder from local model files
    pub fn from_model_id(model_path: &str) -> Result<Self> {
        Self::from_path(model_path)
    }
    
    /// Load CLIP model from a local path
    pub fn from_path(model_path: &str) -> Result<Self> {
        if !Path::new(model_path).exists() {
            return Err(EmbeddingError::ModelLoadFailed.into());
        }
        
        // Use CPU device for now (can be extended to support GPU)
        let device = Device::Cpu;
        
        // Build file paths
        let tokenizer_path = Path::new(model_path).join("tokenizer.json");
        let weights_path = Path::new(model_path).join("model.safetensors")
            .exists()
            .then(|| Path::new(model_path).join("model.safetensors"))
            .or_else(|| {
                Path::new(model_path).join("pytorch_model.bin")
                    .exists()
                    .then(|| Path::new(model_path).join("pytorch_model.bin"))
            })
            .ok_or(EmbeddingError::ModelLoadFailed)?;
        
        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|_| EmbeddingError::TokenizationFailed)?;
        
        // Use the same config as the working version - don't load from JSON
        let config = ClipConfig::vit_base_patch32();
        
        // Load model weights
        let vb = if weights_path.to_string_lossy().ends_with(".safetensors") {
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)
                .map_err(|_| EmbeddingError::ModelLoadFailed)? }
        } else {
            VarBuilder::from_pth(&weights_path, DType::F32, &device)
                .map_err(|_| EmbeddingError::ModelLoadFailed)?
        };
        
        // Create the model
        let model = ClipModel::new(vb, &config)
            .map_err(|_| EmbeddingError::ModelLoadFailed)?;
        
        let embedding_dim = config.text_config.embed_dim;
        
        Ok(Self {
            model,
            tokenizer,
            device,
            embedding_dim,
        })
    }
    
    /// Process image and return embedding tensor
    fn process_image(&self, image_path: &str) -> Result<Tensor> {
        // Use exact same approach as working candle_clip_test.rs
        let img = image::ImageReader::open(image_path)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .decode()
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        let (height, width) = (224, 224);
        let img = img.resize_to_fill(
            width as u32,
            height as u32,
            image::imageops::FilterType::Triangle,
        );
        let img = img.to_rgb8();
        let img = img.into_raw();
        
        // Use Device::Cpu directly like the working version
        let img = Tensor::from_vec(img, (height, width, 3), &Device::Cpu)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .permute((2, 0, 1))
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .to_dtype(DType::F32)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .affine(2. / 255., -1.)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        // Add batch dimension (working version does this in main())
        let img = img.unsqueeze(0)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .to_device(&self.device)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        Ok(img)
    }
    
    /// Process text and return token tensor
    fn process_text(&self, text: &str) -> Result<Tensor> {
        // Tokenize text
        let encoding = self.tokenizer
            .encode(text, true)
            .map_err(|_| EmbeddingError::TokenizationFailed)?;
        
        let tokens = encoding.get_ids();
        let token_ids: Vec<u32> = tokens.iter().map(|&x| x as u32).collect();
        
        // Convert to tensor and add batch dimension
        let tensor = Tensor::from_slice(&token_ids, (1, token_ids.len()), &self.device)
            .map_err(|_| EmbeddingError::TokenizationFailed)?;
        
        Ok(tensor)
    }
    
    /// Convert Candle tensor to ndarray
    fn tensor_to_array(&self, tensor: &Tensor) -> Result<Array1<f64>> {
        // Convert tensor to CPU if needed and get data
        let tensor = match tensor.device() {
            Device::Cpu => tensor.clone(),
            _ => tensor.to_device(&Device::Cpu).map_err(|_| EmbeddingError::ImageProcessingFailed)?,
        };
        
        // Extract values as f32 then convert to f64
        let tensor = if tensor.dims().len() > 1 && tensor.dims()[0] == 1 {
            // Remove batch dimension if present
            tensor.squeeze(0).map_err(|_| EmbeddingError::ImageProcessingFailed)?
        } else {
            tensor
        };
        
        let values: Vec<f32> = tensor.to_vec1().map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        let values_f64: Vec<f64> = values.into_iter().map(|x| x as f64).collect();
        
        Ok(Array1::from_vec(values_f64))
    }
    
    /// Parse CLIP configuration from JSON
    /// Tokenize multiple text sequences for batch processing
    fn tokenize_batch(&self, texts: &[String]) -> Result<Tensor> {
        let pad_id = *self.tokenizer
            .get_vocab(true)
            .get("<|endoftext|>")
            .ok_or(EmbeddingError::TokenizationFailed)?;
        
        let mut tokens = vec![];
        for text in texts {
            let encoding = self.tokenizer.encode(text.as_str(), true)
                .map_err(|_| EmbeddingError::TokenizationFailed)?;
            tokens.push(encoding.get_ids().to_vec());
        }
        
        // Find max length and pad all sequences
        let max_len = tokens.iter().map(|v| v.len()).max().unwrap_or(0);
        for token_vec in tokens.iter_mut() {
            let len_diff = max_len - token_vec.len();
            if len_diff > 0 {
                token_vec.extend(vec![pad_id; len_diff]);
            }
        }
        
        // Create tensor
        let input_ids = Tensor::new(tokens, &self.device)
            .map_err(|_| EmbeddingError::InvalidTensorShape)?;
        
        Ok(input_ids)
    }
}

impl Default for ClipEmbedder {
    fn default() -> Self {
        // For testing purposes, create a minimal ClipEmbedder that will fail on actual embedding calls
        // but allows the struct to be created for testing
        let device = Device::Cpu;
        
        // Create ViT-B/32 configuration manually for testing
        let config = ClipConfig::vit_base_patch32();
        
        // Create minimal structures - these won't work for actual inference
        // but allow the struct to be instantiated for testing
        let vb = VarBuilder::zeros(DType::F32, &device);
        let model = ClipModel::new(vb, &config).unwrap_or_else(|_| {
            // This is a fallback for cases where model creation fails
            // In practice, this would be very rare
            panic!("Failed to create minimal ClipModel for testing")
        });
        
        // Create a minimal tokenizer - this is a simplified approach for testing
        let tokenizer = Tokenizer::new(tokenizers::models::bpe::BPE::default());
        
        Self {
            model,
            tokenizer,
            device,
            embedding_dim: config.text_config.embed_dim,
        }
    }
}

impl EmbedderTrait for ClipEmbedder {
    fn get_image_embedding(&self, image_path: &str) -> Result<Array1<f64>> {
        // Process image and get embedding
        let image_tensor = self.process_image(image_path)?;
        
        // Get image features from the model
        let image_features = self.model.get_image_features(&image_tensor)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        // Convert to array and normalize
        let mut embedding = self.tensor_to_array(&image_features)?;
        
        // Normalize the embedding vector
        let norm = embedding.dot(&embedding).sqrt();
        if norm > 0.0 {
            embedding /= norm;
        }
        
        Ok(embedding)
    }
    
    fn get_text_embedding(&self, text: &str) -> Result<Array1<f64>> {
        // Process text and get embedding
        let text_tensor = self.process_text(text)?;
        
        // Get text features from the model
        let text_features = self.model.get_text_features(&text_tensor)
            .map_err(|_| EmbeddingError::TokenizationFailed)?;
        
        // Convert to array and normalize
        let mut embedding = self.tensor_to_array(&text_features)?;
        
        // Normalize the embedding vector
        let norm = embedding.dot(&embedding).sqrt();
        if norm > 0.0 {
            embedding /= norm;
        }
        
        Ok(embedding)
    }
    
    fn calculate_batch_similarities(&self, image_path: &str, texts: &[String]) -> Result<Vec<f64>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        
        // Process image
        let image_tensor = self.process_image(image_path)?;
        
        // Tokenize all texts in batch
        let text_tensor = self.tokenize_batch(texts)?;
        
        // Use the CLIP model's forward pass (the correct way!)
        let (_logits_per_text, logits_per_image) = self.model.forward(&image_tensor, &text_tensor)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        // Apply softmax to get competitive probabilities
        let softmax_result = softmax(&logits_per_image, 1)
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        // Convert to percentages
        let probabilities = softmax_result.flatten_all()
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?
            .to_vec1::<f32>()
            .map_err(|_| EmbeddingError::ImageProcessingFailed)?;
        
        // Convert to f64 percentages
        let percentages: Vec<f64> = probabilities.iter()
            .map(|&x| (x as f64) * 100.0)
            .collect();
        
        Ok(percentages)
    }
    
    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }
}

/// Calculate cosine similarity between two embedding vectors
/// 
/// # Arguments
/// * `a` - First embedding vector
/// * `b` - Second embedding vector
/// 
/// # Returns
/// Cosine similarity score between -1 and 1
pub fn cosine_similarity(a: &Array1<f64>, b: &Array1<f64>) -> Result<f64> {
    if a.len() != b.len() {
        return Err(EmbeddingError::InvalidDimensions.into());
    }
    
    // For normalized vectors, cosine similarity is just the dot product
    let similarity = a.dot(b);
    
    // Clamp to valid range to handle floating point precision issues
    Ok(similarity.max(-1.0).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mock_embedder_deterministic() {
        let embedder = MockEmbedder::new(128);
        
        let text = "test text";
        let embedding1 = embedder.get_text_embedding(text).unwrap();
        let embedding2 = embedder.get_text_embedding(text).unwrap();
        
        // Should be deterministic
        assert_eq!(embedding1, embedding2);
    }
    
    #[test]
    fn test_mock_embedder_different_inputs() {
        let embedder = MockEmbedder::new(128);
        
        let embedding1 = embedder.get_text_embedding("text1").unwrap();
        let embedding2 = embedder.get_text_embedding("text2").unwrap();
        
        // Different inputs should produce different embeddings
        assert_ne!(embedding1, embedding2);
    }
    
    #[test]
    fn test_mock_embedder_normalized() {
        let embedder = MockEmbedder::new(128);
        
        let embedding = embedder.get_text_embedding("test").unwrap();
        let norm = embedding.dot(&embedding).sqrt();
        
        // Should be approximately normalized (within floating point precision)
        assert!((norm - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_cosine_similarity() {
        let embedder = MockEmbedder::new(128);
        
        let embedding1 = embedder.get_text_embedding("similar text").unwrap();
        let embedding2 = embedder.get_text_embedding("similar text").unwrap();
        let embedding3 = embedder.get_text_embedding("different text").unwrap();
        
        // Identical embeddings should have similarity of 1.0
        let sim_identical = cosine_similarity(&embedding1, &embedding2).unwrap();
        assert!((sim_identical - 1.0).abs() < 1e-10);
        
        // Different embeddings should have different similarity
        let sim_different = cosine_similarity(&embedding1, &embedding3).unwrap();
        assert!(sim_different < 1.0);
        assert!(sim_different >= -1.0);
    }
    
    #[test]
    fn test_cosine_similarity_dimension_mismatch() {
        let embedder1 = MockEmbedder::new(128);
        let embedder2 = MockEmbedder::new(256);
        
        let embedding1 = embedder1.get_text_embedding("test").unwrap();
        let embedding2 = embedder2.get_text_embedding("test").unwrap();
        
        let result = cosine_similarity(&embedding1, &embedding2);
        assert!(matches!(result, Err(crate::error::CliptionsError::Embedding(EmbeddingError::InvalidDimensions))));
    }
    
    #[test]
    fn test_image_vs_text_embeddings() {
        let embedder = MockEmbedder::new(128);
        
        let image_embedding = embedder.get_image_embedding("test.jpg").unwrap();
        let text_embedding = embedder.get_text_embedding("test.jpg").unwrap();
        
        // Same input should produce different embeddings for image vs text
        assert_ne!(image_embedding, text_embedding);
    }
    
    #[test]
    fn test_embedding_dimensions() {
        let embedder = MockEmbedder::new(256);
        
        assert_eq!(embedder.embedding_dim(), 256);
        
        let embedding = embedder.get_text_embedding("test").unwrap();
        assert_eq!(embedding.len(), 256);
    }
    
    #[test]
    fn test_clip_embedder_creation() {
        // Test creation with non-existent model path
        let result = ClipEmbedder::from_path("non_existent_model_path");
        assert!(result.is_err());
        
        // Test default creation (this should now succeed due to automatic downloading)
        let result = ClipEmbedder::new();
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_clip_embedder_default_fallback() {
        // Test that default creation falls back gracefully
        let embedder = ClipEmbedder::default();
        assert_eq!(embedder.embedding_dim(), 512);
        
        // Verify that methods return appropriate errors when Python script is not available
        let result = embedder.get_text_embedding("test text");
        assert!(result.is_err());
        
        let result = embedder.get_image_embedding("test_image.jpg");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_clip_embedder_interface_consistency() {
        // Verify that ClipEmbedder implements the EmbedderTrait properly
        let embedder = ClipEmbedder::default();
        
        // Check that it has the correct embedding dimension
        assert_eq!(embedder.embedding_dim(), 512);
        
        // Verify the trait methods exist and return Results
        let text_result = embedder.get_text_embedding("test");
        assert!(matches!(text_result, Err(_)));
        
        let image_result = embedder.get_image_embedding("test.jpg");
        assert!(matches!(image_result, Err(_)));
    }
}