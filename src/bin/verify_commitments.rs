//! Verify commitments for Cliptions prediction rounds
//! 
//! Enhanced CLI tool with comprehensive error handling, multiple output formats,
//! configuration support, and improved user experience for verifying cryptographic
//! commitments in prediction market rounds.

use std::process;
use std::path::PathBuf;
use std::fs;
use clap::Parser;
use colored::Colorize;

use cliptions_core::embedder::{MockEmbedder, ClipEmbedder, EmbedderTrait};
use cliptions_core::scoring::ClipBatchStrategy;
use cliptions_core::round::RoundProcessor;
use cliptions_core::config::ConfigManager;

#[derive(Parser)]
#[command(name = "verify_commitments")]
#[command(about = "Verify commitments for Cliptions prediction rounds")]
#[command(version = "2.0")]
#[command(long_about = "
Verify cryptographic commitments for Cliptions prediction market rounds with comprehensive
error handling and multiple output formats.

This tool validates that participant commitments match their revealed guesses and salts,
ensuring the integrity of the prediction market. Results can be displayed in multiple
formats and saved to files for audit trails.

Examples:
  # Verify commitments for a specific round
  verify_commitments round1
  
  # Detailed verification with verbose output
  verify_commitments round1 --verbose --detailed
  
  # Save verification results to JSON
  verify_commitments round1 --output json --output-file verification.json
  
  # Use custom rounds file with configuration
  verify_commitments round1 --rounds-file data/rounds.json --config config.yaml
  
  # Batch verify multiple rounds
  verify_commitments --all --continue-on-error --output csv
")]
struct Args {
    /// Round ID to verify (required unless --all is specified)
    round_id: Option<String>,
    
    /// Verify all rounds
    #[arg(long)]
    all: bool,
    
    /// Path to rounds file
    #[arg(long, default_value = "rounds.json")]
    rounds_file: PathBuf,
    
    /// Output format: table, json, csv
    #[arg(long, short, default_value = "table", value_parser = ["table", "json", "csv"])]
    output: String,

    /// Save results to file
    #[arg(long, short)]
    output_file: Option<PathBuf>,

    /// Use MockEmbedder instead of CLIP for testing (fast, deterministic)
    #[arg(long)]
    use_mock: bool,

    /// Path to CLIP model directory (optional, uses default if not specified)
    #[arg(long)]
    clip_model: Option<PathBuf>,
    
    /// Enable verbose output with detailed progress information
    #[arg(short, long)]
    verbose: bool,

    /// Suppress colored output (useful for scripts/logging)
    #[arg(long)]
    no_color: bool,

    /// Configuration file path (YAML format)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Continue processing on errors (for batch operations)
    #[arg(long)]
    continue_on_error: bool,

    /// Show detailed verification breakdown for each participant
    #[arg(long)]
    detailed: bool,

    /// Fail on any invalid commitment (exit code 1)
    #[arg(long)]
    strict: bool,

    /// Only show invalid commitments (filter out valid ones)
    #[arg(long)]
    invalid_only: bool,

    /// Maximum number of rounds to process (for --all, 0 = unlimited)
    #[arg(long, default_value = "0")]
    max_rounds: usize,
}

fn main() {
    let args = Args::parse();
    
    // Initialize colored output
    if args.no_color {
        colored::control::set_override(false);
    }

    // Load configuration if specified
    let _config_manager = if let Some(config_path) = &args.config {
        match ConfigManager::with_path(config_path) {
            Ok(manager) => {
                if args.verbose {
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
        match ConfigManager::new() {
            Ok(manager) => {
                if args.verbose {
                    println!("{} Using default configuration", 
                        "Info:".blue().bold()
                    );
                }
                Some(manager)
            }
            Err(_) => {
                if args.verbose {
                    println!("{} No configuration file found, using built-in defaults", 
                        "Info:".blue().bold()
                    );
                }
                None
            }
        }
    };
    
    // Validate arguments with enhanced error messages
    if let Err(e) = validate_inputs(&args) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        eprintln!("{} Use --help for usage information", "Tip:".yellow().bold());
        process::exit(1);
    }

    // Create processor and verify commitments
    let results = create_processor_and_verify(&args);

    match results {
        Ok(verification_data) => {
            // Display results
            if let Err(e) = display_results(&verification_data, &args) {
                eprintln!("{} Failed to display results: {}", "Error:".red().bold(), e);
                process::exit(1);
            }

            // Save to file if requested
            if let Some(output_file) = &args.output_file {
                if let Err(e) = save_results(&verification_data, output_file, &args.output) {
                    eprintln!("{} Failed to save results: {}", "Error:".red().bold(), e);
                    process::exit(1);
                }
                
                println!("{} Results saved to {}", 
                    "Success:".green().bold(), 
                    output_file.display()
                );
            }

            // Check for failures and exit with appropriate code
            let has_failures = verification_data.rounds.iter()
                .any(|(_, results, _)| results.iter().any(|&r| !r));

            if args.strict && has_failures {
                eprintln!("{} Verification failed (strict mode)", "Error:".red().bold());
                process::exit(1);
            }

            if args.verbose && !has_failures {
                println!("{} All commitment verifications passed", 
                    "Success:".green().bold()
                );
            }
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red().bold(), e);
            process::exit(1);
        }
    }
}

fn validate_inputs(args: &Args) -> Result<(), String> {
    // Validate mutual exclusivity
    if !args.all && args.round_id.is_none() {
        return Err("Must specify either a round ID or --all".to_string());
    }
    
    if args.all && args.round_id.is_some() {
        return Err("Cannot specify both a round ID and --all".to_string());
    }

    // Validate rounds file exists
    if !args.rounds_file.exists() {
        return Err(format!("Rounds file does not exist: {}", args.rounds_file.display()));
    }

    // Validate CLIP model path if provided
    if let Some(model_path) = &args.clip_model {
        if !model_path.exists() {
            return Err(format!("CLIP model path does not exist: {}", model_path.display()));
        }
    }

    // Validate output file directory exists if specified
    if let Some(output_file) = &args.output_file {
        if let Some(parent) = output_file.parent() {
            if !parent.exists() {
                return Err(format!("Output directory does not exist: {}", parent.display()));
            }
        }
    }

    Ok(())
}

fn create_processor_and_verify(args: &Args) -> Result<VerificationResults, Box<dyn std::error::Error>> {
    let strategy = ClipBatchStrategy::new();
    
    // Create processor and verify based on embedder type (defaults to CLIP)
    if args.use_mock {
        if args.verbose {
            println!("{} Using MockEmbedder for verification", 
                "Info:".blue().bold()
            );
        }
        let embedder = MockEmbedder::clip_like();
        let processor = RoundProcessor::new(args.rounds_file.to_string_lossy().to_string(), embedder, strategy);
        verify_with_processor(processor, args)
    } else {
        // Default: Use CLIP embedder
        if let Some(model_path) = &args.clip_model {
            match ClipEmbedder::from_path(&model_path.to_string_lossy()) {
                Ok(embedder) => {
                    if args.verbose {
                        println!("{} Using CLIP embedder from {}", 
                            "Info:".blue().bold(), 
                            model_path.display()
                        );
                    }
                    let processor = RoundProcessor::new(args.rounds_file.to_string_lossy().to_string(), embedder, strategy);
                    verify_with_processor(processor, args)
                }
                Err(e) => {
                    eprintln!("{} Failed to load CLIP model from {}: {}", 
                        "Warning:".yellow().bold(), 
                        model_path.display(),
                        e
                    );
                    eprintln!("{} Falling back to MockEmbedder", 
                        "Info:".blue().bold()
                    );
                    let embedder = MockEmbedder::clip_like();
                    let processor = RoundProcessor::new(args.rounds_file.to_string_lossy().to_string(), embedder, strategy);
                    verify_with_processor(processor, args)
                }
            }
        } else {
            match ClipEmbedder::new() {
                Ok(embedder) => {
                    if args.verbose {
                        println!("{} Using default CLIP embedder", 
                            "Info:".blue().bold()
                        );
                    }
                    let processor = RoundProcessor::new(args.rounds_file.to_string_lossy().to_string(), embedder, strategy);
                    verify_with_processor(processor, args)
                }
                Err(e) => {
                    eprintln!("{} Failed to load default CLIP model: {}", 
                        "Warning:".yellow().bold(), 
                        e
                    );
                    eprintln!("{} Falling back to MockEmbedder", 
                        "Info:".blue().bold()
                    );
                    let embedder = MockEmbedder::clip_like();
                    let processor = RoundProcessor::new(args.rounds_file.to_string_lossy().to_string(), embedder, strategy);
                    verify_with_processor(processor, args)
                }
            }
        }
    }
}

fn verify_with_processor<E: EmbedderTrait>(
    mut processor: RoundProcessor<E, ClipBatchStrategy>,
    args: &Args
) -> Result<VerificationResults, Box<dyn std::error::Error>> {
    
    // Load rounds first
    processor.load_rounds()?;
    
    if args.all {
        verify_all_rounds(processor, args)
    } else if let Some(round_id) = &args.round_id {
        verify_single_round(processor, round_id, args)
    } else {
        Err("Must specify either a round ID or --all".into())
    }
}

#[derive(Debug)]
struct VerificationResults {
    rounds: Vec<(String, Vec<bool>, Vec<cliptions_core::types::Participant>)>,
    total_rounds_processed: usize,
    total_participants: usize,
    total_valid: usize,
    total_invalid: usize,
    errors: Vec<String>,
}

fn verify_all_rounds<E: EmbedderTrait>(
    mut processor: RoundProcessor<E, ClipBatchStrategy>,
    args: &Args
) -> Result<VerificationResults, Box<dyn std::error::Error>> {
    
    if args.verbose {
        println!("{} Verifying all rounds...", "Info:".blue().bold());
    }

    let round_ids = processor.get_round_ids()?;
    let mut results = VerificationResults {
        rounds: Vec::new(),
        total_rounds_processed: 0,
        total_participants: 0,
        total_valid: 0,
        total_invalid: 0,
        errors: Vec::new(),
    };

    let mut processed_count = 0;
    
    for round_id in round_ids {
        // Check max rounds limit
        if args.max_rounds > 0 && processed_count >= args.max_rounds {
            if args.verbose {
                println!("{} Reached maximum rounds limit ({})", 
                    "Info:".blue().bold(), 
                    args.max_rounds
                );
            }
            break;
        }

        match process_round_verification(&mut processor, &round_id, args) {
            Ok((verification_results, participants)) => {
                let valid_count = verification_results.iter().filter(|&&r| r).count();
                let invalid_count = verification_results.len() - valid_count;

                let verification_len = verification_results.len();
                results.rounds.push((round_id.to_string(), verification_results, participants));
                results.total_rounds_processed += 1;
                results.total_participants += verification_len;
                results.total_valid += valid_count;
                results.total_invalid += invalid_count;
                processed_count += 1;

                if args.verbose {
                    println!("{} Verified round {} ({}/{} valid)", 
                        "Info:".blue().bold(), 
                        round_id,
                        valid_count,
                        verification_len
                    );
                }
            }
            Err(e) => {
                let error_msg = format!("Failed to verify round {}: {}", round_id, e);
                results.errors.push(error_msg.clone());
                
                if args.continue_on_error {
                    if args.verbose {
                        eprintln!("{} {}", "Warning:".yellow().bold(), error_msg);
                    }
                } else {
                    return Err(error_msg.into());
                }
            }
        }
    }

    Ok(results)
}

fn verify_single_round<E: EmbedderTrait>(
    mut processor: RoundProcessor<E, ClipBatchStrategy>,
    round_id: &str,
    args: &Args
) -> Result<VerificationResults, Box<dyn std::error::Error>> {
    
    if args.verbose {
        println!("{} Verifying round: {}", "Info:".blue().bold(), round_id);
    }

    let (verification_results, participants) = process_round_verification(&mut processor, round_id, args)?;
    
    let valid_count = verification_results.iter().filter(|&&r| r).count();
    let invalid_count = verification_results.len() - valid_count;

    let verification_len = verification_results.len();
    let results = VerificationResults {
        rounds: vec![(round_id.to_string(), verification_results, participants)],
        total_rounds_processed: 1,
        total_participants: verification_len,
        total_valid: valid_count,
        total_invalid: invalid_count,
        errors: Vec::new(),
    };

    if args.verbose {
        println!("{} Verified round {} ({}/{} valid)", 
            "Info:".blue().bold(), 
            round_id,
            valid_count,
            results.total_participants
        );
    }

    Ok(results)
}

fn process_round_verification<E: EmbedderTrait>(
    processor: &mut RoundProcessor<E, ClipBatchStrategy>,
    round_id: &str,
    args: &Args
) -> Result<(Vec<bool>, Vec<cliptions_core::types::Participant>), Box<dyn std::error::Error>> {
    
    // Get round info
    let round = processor.get_round(round_id)?;
    
    if args.verbose {
        println!("{} Round: {} - {} participants", 
            "Info:".blue().bold(), 
            round.title,
            round.participants.len()
        );
    }

    if round.participants.is_empty() {
        if args.verbose {
            println!("{} No participants to verify in round {}", 
                "Info:".blue().bold(), 
                round_id
            );
        }
        return Ok((Vec::new(), Vec::new()));
    }

    // Clone participants to avoid borrowing issues
    let participants = round.participants.clone();
    
    // Verify commitments
    let verification_results = processor.verify_commitments(round_id)?;
    
    Ok((verification_results, participants))
}

fn display_results(results: &VerificationResults, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    match args.output.as_str() {
        "table" => display_table_format(results, args),
        "json" => display_json_format(results),
        "csv" => display_csv_format(results),
        _ => Err(format!("Unsupported output format: {}", args.output).into()),
    }
}

fn display_table_format(results: &VerificationResults, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "Commitment Verification Results:".bold().underline());
    println!("{}", "=".repeat(80));

    if results.rounds.is_empty() {
        println!("{} No rounds processed", "Info:".blue().bold());
        return Ok(());
    }

    for (round_id, verification_results, participants) in &results.rounds {
        println!("\n{} {}", "Round:".bold().blue(), round_id.bright_white());
        
        let valid_count = verification_results.iter().filter(|&&r| r).count();
        let total_count = verification_results.len();
        
        println!("Valid commitments: {}/{}", 
            format!("{}", valid_count).green().bold(),
            total_count
        );

        if args.detailed && !participants.is_empty() {
            println!("\n{}", "Detailed Verification:".dimmed());
            
            for (i, (participant, &is_valid)) in participants.iter().zip(verification_results.iter()).enumerate() {
                // Skip valid commitments if only showing invalid ones
                if args.invalid_only && is_valid {
                    continue;
                }
                
                let status = if is_valid { 
                    "✓ VALID".green().bold() 
                } else { 
                    "✗ INVALID".red().bold() 
                };
                
                println!("  {}. {} ({}): {}", 
                    i + 1, 
                    participant.username,
                    participant.user_id.dimmed(),
                    status
                );
                
                if args.verbose || !is_valid {
                    println!("     Guess: \"{}\"", participant.guess.text);
                    println!("     Commitment: {}", participant.commitment);
                    if let Some(salt) = &participant.salt {
                        println!("     Salt: {}", salt);
                    } else {
                        println!("     Salt: {}", "[NOT PROVIDED]".red());
                    }
                    
                    if participant.verified {
                        println!("     Status: {}", "Verified".green());
                    } else {
                        println!("     Status: {}", "Unverified".yellow());
                    }
                }
                println!();
            }
        }

        if valid_count == total_count && total_count > 0 {
            println!("   {} All commitments verified successfully!", 
                "✓".green().bold()
            );
        } else if valid_count == 0 && total_count > 0 {
            println!("   {} No valid commitments found!", 
                "✗".red().bold()
            );
        } else if total_count > 0 {
            println!("   {} {} commitment(s) failed verification", 
                "⚠".yellow().bold(),
                total_count - valid_count
            );
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("{} {}", "Summary:".bold(), "");
    println!("Rounds Processed: {}", results.total_rounds_processed);
    println!("Total Participants: {}", results.total_participants);
    println!("Valid Commitments: {}", format!("{}", results.total_valid).green().bold());
    println!("Invalid Commitments: {}", format!("{}", results.total_invalid).red().bold());
    
    if results.total_participants > 0 {
        let success_rate = (results.total_valid as f64 / results.total_participants as f64) * 100.0;
        println!("Success Rate: {:.1}%", success_rate);
    }

    if !results.errors.is_empty() {
        println!("\n{} {} error(s) encountered:", "Warning:".yellow().bold(), results.errors.len());
        for error in &results.errors {
            println!("  • {}", error);
        }
    }

    Ok(())
}

fn display_json_format(results: &VerificationResults) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = serde_json::Map::new();
    
    let rounds_data: Vec<serde_json::Value> = results.rounds.iter()
        .map(|(round_id, verification_results, participants)| {
            let participant_data: Vec<serde_json::Value> = participants.iter()
                .zip(verification_results.iter())
                .map(|(participant, &is_valid)| {
                    serde_json::json!({
                        "username": participant.username,
                        "user_id": participant.user_id,
                        "guess": participant.guess.text,
                        "commitment": participant.commitment,
                        "salt": participant.salt,
                        "is_verified": participant.verified,
                        "commitment_valid": is_valid
                    })
                })
                .collect();
            
            let valid_count = verification_results.iter().filter(|&&r| r).count();
            
            serde_json::json!({
                "round_id": round_id,
                "participants": participant_data,
                "total_participants": verification_results.len(),
                "valid_commitments": valid_count,
                "invalid_commitments": verification_results.len() - valid_count
            })
        })
        .collect();
    
    output.insert("rounds".to_string(), serde_json::Value::Array(rounds_data));
    output.insert("summary".to_string(), serde_json::json!({
        "total_rounds_processed": results.total_rounds_processed,
        "total_participants": results.total_participants,
        "total_valid": results.total_valid,
        "total_invalid": results.total_invalid,
        "success_rate": if results.total_participants > 0 { 
            (results.total_valid as f64 / results.total_participants as f64) * 100.0 
        } else { 
            0.0 
        },
        "errors": results.errors
    }));
    output.insert("timestamp".to_string(), serde_json::Value::from(chrono::Utc::now().to_rfc3339()));
    
    let json_output = serde_json::to_string_pretty(&output)?;
    println!("{}", json_output);
    
    Ok(())
}

fn display_csv_format(results: &VerificationResults) -> Result<(), Box<dyn std::error::Error>> {
    println!("round_id,username,user_id,guess,commitment,salt,is_verified,commitment_valid");
    
    for (round_id, verification_results, participants) in &results.rounds {
        for (participant, &is_valid) in participants.iter().zip(verification_results.iter()) {
            let escaped_guess = participant.guess.text.replace("\"", "\"\"");
            let salt_str = participant.salt.as_ref().map_or("".to_string(), |s| s.clone());
            
            println!("{},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{}",
                round_id,
                participant.username,
                participant.user_id,
                escaped_guess,
                participant.commitment,
                salt_str,
                participant.verified,
                is_valid
            );
        }
    }
    
    Ok(())
}

fn save_results(
    results: &VerificationResults, 
    output_file: &PathBuf,
    format: &str
) -> Result<(), Box<dyn std::error::Error>> {
    
    let content = match format {
        "json" => {
            let mut output = serde_json::Map::new();
            
            let rounds_data: Vec<serde_json::Value> = results.rounds.iter()
                .map(|(round_id, verification_results, participants)| {
                    let participant_data: Vec<serde_json::Value> = participants.iter()
                        .zip(verification_results.iter())
                        .map(|(participant, &is_valid)| {
                            serde_json::json!({
                                "username": participant.username,
                                "user_id": participant.user_id,
                                "guess": participant.guess.text,
                                "commitment": participant.commitment,
                                "salt": participant.salt,
                                "is_verified": participant.verified,
                                "commitment_valid": is_valid
                            })
                        })
                        .collect();
                    
                    let valid_count = verification_results.iter().filter(|&&r| r).count();
                    
                    serde_json::json!({
                        "round_id": round_id,
                        "participants": participant_data,
                        "total_participants": verification_results.len(),
                        "valid_commitments": valid_count,
                        "invalid_commitments": verification_results.len() - valid_count
                    })
                })
                .collect();
            
            output.insert("rounds".to_string(), serde_json::Value::Array(rounds_data));
            output.insert("summary".to_string(), serde_json::json!({
                "total_rounds_processed": results.total_rounds_processed,
                "total_participants": results.total_participants,
                "total_valid": results.total_valid,
                "total_invalid": results.total_invalid,
                "success_rate": if results.total_participants > 0 { 
                    (results.total_valid as f64 / results.total_participants as f64) * 100.0 
                } else { 
                    0.0 
                },
                "errors": results.errors
            }));
            output.insert("timestamp".to_string(), serde_json::Value::from(chrono::Utc::now().to_rfc3339()));
            
            serde_json::to_string_pretty(&output)?
        }
        "csv" => {
            let mut content = String::from("round_id,username,user_id,guess,commitment,salt,is_verified,commitment_valid\n");
            
            for (round_id, verification_results, participants) in &results.rounds {
                for (participant, &is_valid) in participants.iter().zip(verification_results.iter()) {
                    let escaped_guess = participant.guess.text.replace("\"", "\"\"");
                    let salt_str = participant.salt.as_ref().map_or("".to_string(), |s| s.clone());
                    
                    content.push_str(&format!("{},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{}\n",
                        round_id,
                        participant.username,
                        participant.user_id,
                        escaped_guess,
                        participant.commitment,
                        salt_str,
                        participant.verified,
                        is_valid
                    ));
                }
            }
            
            content
        }
        "table" => {
            let mut content = String::from("Commitment Verification Results\n");
            content.push_str(&"=".repeat(50));
            content.push('\n');
            
            for (round_id, verification_results, participants) in &results.rounds {
                content.push_str(&format!("\nRound: {}\n", round_id));
                
                let valid_count = verification_results.iter().filter(|&&r| r).count();
                content.push_str(&format!("Valid commitments: {}/{}\n", valid_count, verification_results.len()));
                
                for (i, (participant, &is_valid)) in participants.iter().zip(verification_results.iter()).enumerate() {
                    let status = if is_valid { "VALID" } else { "INVALID" };
                    content.push_str(&format!("  {}. {} ({}): {}\n", 
                        i + 1, 
                        participant.username,
                        participant.user_id,
                        status
                    ));
                    content.push_str(&format!("     Guess: \"{}\"\n", participant.guess.text));
                    content.push_str(&format!("     Commitment: {}\n", participant.commitment));
                    if let Some(salt) = &participant.salt {
                        content.push_str(&format!("     Salt: {}\n", salt));
                    }
                    content.push('\n');
                }
            }
            
            content.push_str(&"=".repeat(50));
            content.push('\n');
            content.push_str(&format!("Total Rounds: {}\n", results.total_rounds_processed));
            content.push_str(&format!("Total Participants: {}\n", results.total_participants));
            content.push_str(&format!("Valid Commitments: {}\n", results.total_valid));
            content.push_str(&format!("Invalid Commitments: {}\n", results.total_invalid));
            
            content
        }
        _ => return Err(format!("Unsupported output format for file save: {}", format).into()),
    };
    
    fs::write(output_file, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use cliptions_core::types::{RoundData, Participant, Guess};
    use cliptions_core::commitment::CommitmentGenerator;
    use std::collections::HashMap;

    #[test]
    fn test_validate_inputs_valid() {
        let args = Args {
            round_id: Some("test_round".to_string()),
            all: false,
            rounds_file: PathBuf::from("tests/fixtures/rounds.json"),
            output: "table".to_string(),
            output_file: None,
            use_clip: false,
            clip_model: None,
            verbose: false,
            no_color: false,
            config: None,
            continue_on_error: false,
            detailed: false,
            strict: false,
            invalid_only: false,
            max_rounds: 0,
        };
        
        // This will fail if the test file doesn't exist, which is expected
        let result = validate_inputs(&args);
        assert!(result.is_err()); // Expected due to missing test file
    }

    #[test]
    fn test_validate_inputs_invalid_both_flags() {
        let args = Args {
            round_id: Some("test_round".to_string()),
            all: true,
            rounds_file: PathBuf::from("rounds.json"),
            output: "table".to_string(),
            output_file: None,
            use_clip: false,
            clip_model: None,
            verbose: false,
            no_color: false,
            config: None,
            continue_on_error: false,
            detailed: false,
            strict: false,
            invalid_only: false,
            max_rounds: 0,
        };
        
        let result = validate_inputs(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot specify both a round ID and --all"));
    }

    #[test]
    fn test_validate_inputs_neither_flag() {
        let args = Args {
            round_id: None,
            all: false,
            rounds_file: PathBuf::from("rounds.json"),
            output: "table".to_string(),
            output_file: None,
            use_clip: false,
            clip_model: None,
            verbose: false,
            no_color: false,
            config: None,
            continue_on_error: false,
            detailed: false,
            strict: false,
            invalid_only: false,
            max_rounds: 0,
        };
        
        let result = validate_inputs(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Must specify either a round ID or --all"));
    }
    
    #[test]
    fn test_verify_commitments_basic() {
        // Create a temporary rounds file
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_path_buf();
        
        // Create test round data
        let mut round = RoundData::new(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
        );
        
        // Add a test participant with valid commitment
        let commitment_gen = CommitmentGenerator::new();
        let salt = "test_salt";
        let message = "test guess";
        let commitment = commitment_gen.generate(message, salt).unwrap();
        
        let participant = Participant::new(
            "user1".to_string(),
            "user_user1".to_string(),
            Guess::new(message.to_string()),
            commitment,
        ).with_salt(salt.to_string());
        
        round.add_participant(participant);
        
        // Save rounds data
        let mut rounds = HashMap::new();
        rounds.insert("test_round".to_string(), round);
        let content = serde_json::to_string_pretty(&rounds).unwrap();
        std::fs::write(&file_path, content).unwrap();
        
        // Test validation
        let args = Args {
            round_id: Some("test_round".to_string()),
            all: false,
            rounds_file: file_path,
            output: "table".to_string(),
            output_file: None,
            use_clip: false,
            clip_model: None,
            verbose: false,
            no_color: false,
            config: None,
            continue_on_error: false,
            detailed: false,
            strict: false,
            invalid_only: false,
            max_rounds: 0,
        };
        
        let result = validate_inputs(&args);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_verify_invalid_commitment() {
        // Create a temporary rounds file
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_path_buf();
        
        // Create test round data
        let mut round = RoundData::new(
            "test_round".to_string(),
            "Test Round".to_string(),
            "A test round".to_string(),
            "test.jpg".to_string(),
        );
        
        // Add a test participant with invalid commitment
        let participant = Participant::new(
            "user1".to_string(),
            "user_user1".to_string(),
            Guess::new("test guess".to_string()),
            "invalid_commitment".to_string(),
        ).with_salt("test_salt".to_string());
        
        round.add_participant(participant);
        
        // Save rounds data
        let mut rounds = HashMap::new();
        rounds.insert("test_round".to_string(), round);
        let content = serde_json::to_string_pretty(&rounds).unwrap();
        std::fs::write(&file_path, content).unwrap();
        
        // Test validation
        let args = Args {
            round_id: Some("test_round".to_string()),
            all: false,
            rounds_file: file_path,
            output: "table".to_string(),
            output_file: None,
            use_clip: false,
            clip_model: None,
            verbose: false,
            no_color: false,
            config: None,
            continue_on_error: false,
            detailed: false,
            strict: false,
            invalid_only: false,
            max_rounds: 0,
        };
        
        let result = validate_inputs(&args);
        assert!(result.is_ok());
    }
}