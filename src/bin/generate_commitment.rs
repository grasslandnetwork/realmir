//! Generate cryptographic commitments for Cliptions predictions
//! 
//! Enhanced CLI tool for generating secure commitment hashes from prediction messages
//! and salt values. This tool replaces the Python generate_commitment.py script with
//! a native Rust implementation for better performance and integration.

use std::process;
use std::path::PathBuf;
use std::fs;
use clap::Parser;
use colored::Colorize;
use serde_json::Value;

use cliptions_core::commitment::CommitmentGenerator;
use cliptions_core::config::ConfigManager;

#[derive(Parser)]
#[command(name = "generate_commitment")]
#[command(about = "Generate cryptographic commitments for Cliptions predictions")]
#[command(version = "2.0")]
#[command(long_about = "
Generate secure commitment hashes for Cliptions prediction market participation.

This tool creates SHA-256 commitment hashes from your prediction message and a salt value.
The commitment hash can be submitted publicly without revealing your actual prediction,
ensuring fair gameplay in the prediction market.

Examples:
  # Basic commitment generation
  generate_commitment \"Cat sanctuary with woman wearing snoopy sweater\" --salt \"random_secret_123\"
  
  # Save commitment and salt to file for later reference
  generate_commitment \"My prediction\" --salt \"mysalt\" --save-to predictions.json
  
  # Generate multiple commitments from JSON input
  generate_commitment --batch-file commitments.json
  
  # Quiet mode (only output the hash)
  generate_commitment \"My prediction\" --salt \"mysalt\" --quiet
")]
struct Args {
    /// Prediction message to commit to
    message: Option<String>,
    
    /// Salt value for the commitment (required)
    #[arg(long, short)]
    salt: Option<String>,
    
    /// Output format: text, json, csv
    #[arg(long, short, default_value = "text", value_parser = ["text", "json", "csv"])]
    output: String,

    /// Save commitment data to file (JSON format)
    #[arg(long)]
    save_to: Option<PathBuf>,

    /// Batch process commitments from JSON file
    #[arg(long)]
    batch_file: Option<PathBuf>,
    
    /// Enable verbose output with detailed information
    #[arg(short, long)]
    verbose: bool,

    /// Suppress colored output (useful for scripts/logging)
    #[arg(long)]
    no_color: bool,

    /// Quiet mode - only output the commitment hash
    #[arg(short, long)]
    quiet: bool,

    /// Configuration file path (YAML format)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Include timestamp in output
    #[arg(long)]
    timestamp: bool,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CommitmentData {
    message: String,
    salt: String,
    commitment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

#[derive(serde::Serialize)]
struct CommitmentResults {
    commitments: Vec<CommitmentData>,
    total_generated: usize,
}

fn main() {
    let args = Args::parse();
    
    // Initialize colored output
    if args.no_color || args.quiet {
        colored::control::set_override(false);
    }

    // Load configuration if specified
    let _config_manager = if let Some(config_path) = &args.config {
        match ConfigManager::with_path(config_path) {
            Ok(manager) => {
                if args.verbose && !args.quiet {
                    println!("{} Loaded configuration from {}", 
                        "Info:".blue().bold(), 
                        config_path.display()
                    );
                }
                Some(manager)
            }
            Err(e) => {
                eprintln!("{} Failed to load config from {}: {}", 
                    "Error:".red().bold(), 
                    config_path.display(), 
                    e
                );
                process::exit(1);
            }
        }
    } else {
        None
    };
    
    // Validate arguments
    if let Err(e) = validate_inputs(&args) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        eprintln!("{} Use --help for usage information", "Tip:".yellow().bold());
        process::exit(1);
    }

    // Generate commitments
    let results = if args.batch_file.is_some() {
        generate_batch_commitments(&args)
    } else {
        generate_single_commitment(&args)
    };

    match results {
        Ok(commitment_data) => {
            // Display results
            if let Err(e) = display_results(&commitment_data, &args) {
                eprintln!("{} Failed to display results: {}", "Error:".red().bold(), e);
                process::exit(1);
            }

            // Save to file if requested
            if let Some(save_path) = &args.save_to {
                if let Err(e) = save_results(&commitment_data, save_path) {
                    eprintln!("{} Failed to save results: {}", "Error:".red().bold(), e);
                    process::exit(1);
                }
                
                if !args.quiet {
                    println!("{} Commitment data saved to {}", 
                        "Success:".green().bold(), 
                        save_path.display()
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("{} Failed to generate commitment: {}", "Error:".red().bold(), e);
            process::exit(1);
        }
    }
}

fn validate_inputs(args: &Args) -> Result<(), String> {
    // Batch mode validation
    if args.batch_file.is_some() {
        if args.message.is_some() || args.salt.is_some() {
            return Err("Cannot specify message or salt when using --batch-file".to_string());
        }
        return Ok(());
    }

    // Single commitment validation
    if args.message.is_none() {
        return Err("Message is required (unless using --batch-file)".to_string());
    }

    if args.salt.is_none() {
        return Err("--salt is required".to_string());
    }

    // Check message is not empty
    if let Some(ref message) = args.message {
        if message.trim().is_empty() {
            return Err("Message cannot be empty".to_string());
        }
    }

    // Check salt is not empty
    if let Some(ref salt) = args.salt {
        if salt.is_empty() {
            return Err("Salt cannot be empty".to_string());
        }
    }

    Ok(())
}

fn generate_single_commitment(args: &Args) -> Result<CommitmentResults, Box<dyn std::error::Error>> {
    let generator = CommitmentGenerator::new();
    
    let message = args.message.as_ref().unwrap();
    let salt = args.salt.as_ref().unwrap().clone();

    if args.verbose && !args.quiet {
        println!("{} Generating commitment for message: {}", 
            "Info:".blue().bold(), 
            message.chars().take(50).collect::<String>() + if message.len() > 50 { "..." } else { "" }
        );
    }

    let commitment = generator.generate(message, &salt)?;
    
    let timestamp = if args.timestamp {
        Some(chrono::Utc::now().to_rfc3339())
    } else {
        None
    };

    let commitment_data = CommitmentData {
        message: message.clone(),
        salt,
        commitment,
        timestamp,
    };

    Ok(CommitmentResults {
        commitments: vec![commitment_data],
        total_generated: 1,
    })
}

fn generate_batch_commitments(args: &Args) -> Result<CommitmentResults, Box<dyn std::error::Error>> {
    let batch_file = args.batch_file.as_ref().unwrap();
    let file_content = fs::read_to_string(batch_file)?;
    let batch_data: Value = serde_json::from_str(&file_content)?;

    let generator = CommitmentGenerator::new();
    let mut commitments = Vec::new();

    if let Some(batch_array) = batch_data.as_array() {
        for (index, item) in batch_array.iter().enumerate() {
            let message = item.get("message")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("Missing or invalid 'message' field in batch item {}", index))?;

            let salt = item.get("salt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("Missing or invalid 'salt' field in batch item {}", index))?
                .to_string();

            let commitment = generator.generate(message, &salt)?;
            
            let timestamp = if args.timestamp {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                None
            };

            commitments.push(CommitmentData {
                message: message.to_string(),
                salt,
                commitment,
                timestamp,
            });
        }
    } else {
        return Err("Batch file must contain a JSON array".into());
    }

    if args.verbose && !args.quiet {
        println!("{} Generated {} commitments from batch file", 
            "Info:".blue().bold(), 
            commitments.len()
        );
    }

    Ok(CommitmentResults {
        total_generated: commitments.len(),
        commitments,
    })
}

fn display_results(results: &CommitmentResults, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    match args.output.as_str() {
        "text" => display_text_format(results, args),
        "json" => display_json_format(results),
        "csv" => display_csv_format(results),
        _ => unreachable!("Invalid output format should be caught by clap"),
    }
}

fn display_text_format(results: &CommitmentResults, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.quiet {
        // In quiet mode, only output the commitment hash(es)
        for commitment_data in &results.commitments {
            println!("{}", commitment_data.commitment);
        }
        return Ok(());
    }

    if results.commitments.len() == 1 {
        let data = &results.commitments[0];
        
        if args.verbose {
            println!("{}", "Commitment Generation Results".bold().underline());
            println!("{}: {}", "Message".blue().bold(), data.message);
            println!("{}: {}", "Salt".blue().bold(), data.salt);
            println!("{}: {}", "Commitment".green().bold(), data.commitment);
            
            if let Some(ref timestamp) = data.timestamp {
                println!("{}: {}", "Timestamp".blue().bold(), timestamp);
            }
        } else {
            // Simple format matching the original Python script
            println!("Commitment: {}", data.commitment);
        }
    } else {
        // Batch mode
        println!("{}", format!("Generated {} Commitments", results.total_generated).bold().underline());
        println!();
        
        for (index, data) in results.commitments.iter().enumerate() {
            println!("{}{}:", "Commitment ".blue().bold(), (index + 1).to_string().blue().bold());
            println!("  Message: {}", data.message.chars().take(60).collect::<String>() + 
                     if data.message.len() > 60 { "..." } else { "" });
            println!("  Salt: {}...", data.salt.chars().take(16).collect::<String>());
            println!("  Hash: {}", data.commitment.green());
            
            if let Some(ref timestamp) = data.timestamp {
                println!("  Timestamp: {}", timestamp);
            }
            println!();
        }
    }

    Ok(())
}

fn display_json_format(results: &CommitmentResults) -> Result<(), Box<dyn std::error::Error>> {
    let json_output = serde_json::to_string_pretty(results)?;
    println!("{}", json_output);
    Ok(())
}

fn display_csv_format(results: &CommitmentResults) -> Result<(), Box<dyn std::error::Error>> {
    println!("message,salt,commitment,timestamp");
    
    for data in &results.commitments {
        println!("{},{},{},{}", 
            csv_escape(&data.message),
            csv_escape(&data.salt),
            data.commitment,
            data.timestamp.as_deref().unwrap_or("")
        );
    }
    
    Ok(())
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn save_results(results: &CommitmentResults, save_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let json_output = serde_json::to_string_pretty(results)?;
    fs::write(save_path, json_output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_inputs_valid_with_salt() {
        let args = Args {
            message: Some("test message".to_string()),
            salt: Some("test_salt".to_string()),
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        assert!(validate_inputs(&args).is_ok());
    }

    #[test]
    fn test_validate_inputs_missing_salt() {
        let args = Args {
            message: Some("test message".to_string()),
            salt: None,
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        assert!(validate_inputs(&args).is_err());
    }

    #[test]
    fn test_validate_inputs_missing_message() {
        let args = Args {
            message: None,
            salt: Some("test_salt".to_string()),
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        assert!(validate_inputs(&args).is_err());
    }

    #[test]
    fn test_validate_inputs_empty_message() {
        let args = Args {
            message: Some("".to_string()),
            salt: Some("test_salt".to_string()),
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        assert!(validate_inputs(&args).is_err());
    }

    #[test]
    fn test_validate_inputs_empty_salt() {
        let args = Args {
            message: Some("test message".to_string()),
            salt: Some("".to_string()),
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        assert!(validate_inputs(&args).is_err());
    }

    #[test]
    fn test_generate_single_commitment() {
        let args = Args {
            message: Some("test message".to_string()),
            salt: Some("test_salt".to_string()),
            output: "text".to_string(),
            save_to: None,
            batch_file: None,
            verbose: false,
            no_color: false,
            quiet: false,
            config: None,
            timestamp: false,
        };
        
        let result = generate_single_commitment(&args).unwrap();
        assert_eq!(result.total_generated, 1);
        assert_eq!(result.commitments.len(), 1);
        assert_eq!(result.commitments[0].message, "test message");
        assert_eq!(result.commitments[0].salt, "test_salt");
        assert_eq!(result.commitments[0].commitment.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_csv_escape() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("with,comma"), "\"with,comma\"");
        assert_eq!(csv_escape("with\"quote"), "\"with\"\"quote\"");
        assert_eq!(csv_escape("with\nline"), "\"with\nline\"");
    }
} 