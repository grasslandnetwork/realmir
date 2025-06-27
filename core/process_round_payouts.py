#!/usr/bin/env python3
import os
import json
import sys
from pathlib import Path
import argparse
from .calculate_scores_payout import ScoreValidator, calculate_payouts
import numpy as np
from .verify_commitments import verify_round_commitments
from .interfaces import IScoreValidator, IEmbedder
from .scoring_strategies import RawSimilarityStrategy

class LegacyScoreValidator(IScoreValidator):
    """Replicates the scoring logic used before baseline adjustment was added.
    
    This version matches the original scoring used for round0, which:
    - Did not use baseline adjustment
    - Did not have length filtering
    - Did not apply special character penalties
    """
    
    def __init__(self, embedder=None, scoring_strategy=None):
        """Initialize the legacy score validator.
        
        Args:
            embedder: Optional implementation of IEmbedder (defaults to ClipEmbedder)
            scoring_strategy: Optional implementation of IScoringStrategy 
                            (defaults to RawSimilarityStrategy)
        """
        if embedder is None:
            from .clip_embedder import ClipEmbedder
            embedder = ClipEmbedder()
        
        self.embedder = embedder
        self.scoring_strategy = scoring_strategy or RawSimilarityStrategy()
    
    def validate_guess(self, guess: str) -> bool:
        """Simple validation only checks if a guess is a non-empty string."""
        return bool(guess and isinstance(guess, str) and len(guess.strip()) > 0)
    
    def calculate_adjusted_score(self, image_features, guess: str) -> float:
        """Original scoring without baseline adjustment or penalties."""
        if not self.validate_guess(guess):
            return 0.0
            
        # Encode text
        text_features = self.embedder.get_text_embedding(guess)
        
        # Use the strategy to calculate the score
        return self.scoring_strategy.calculate_score(
            image_features=image_features,
            text_features=text_features
        )

def get_validator_for_round(round_id, versions_file="scoring_versions.json"):
    """Get the appropriate validator for the round based on the version registry."""
    try:
        if Path(versions_file).exists():
            with open(versions_file, 'r') as f:
                versions_data = json.load(f)
                
            # Find which version applies to this round
            for version, info in versions_data["versions"].items():
                if round_id in info["applied_to_rounds"]:
                    # Use legacy validator for versions before baseline adjustment
                    if not info["parameters"].get("use_baseline_adjustment", False):
                        print(f"Using legacy scorer (version {version}) for {round_id}")
                        return LegacyScoreValidator()
    except Exception as e:
        print(f"Warning: Could not determine scoring version: {e}")
        print("Defaulting to current validator")
    
    # Default to current validator
    return ScoreValidator()

def process_round_payouts(round_id, prize_pool=None, save_to_file=True, verify_commitments=True):
    """Process payouts for a specific round.
    
    Args:
        round_id: The ID of the round to process (e.g., 'round1')
        prize_pool: Total amount to distribute (optional if already defined in the round data)
        save_to_file: Whether to save results back to guesses.json
        verify_commitments: Whether to verify commitments before processing
        
    Returns:
        Dictionary with round data including calculated scores and payouts
    """
    # Load guesses
    guesses_file = Path("rounds/guesses.json")
    if not guesses_file.exists():
        print(f"Error: {guesses_file} does not exist")
        sys.exit(1)
        
    with open(guesses_file, 'r') as f:
        round_data = json.load(f)
    
    if round_id not in round_data:
        print(f"Error: Round {round_id} not found in guesses.json")
        sys.exit(1)
    
    # Check for existing prize pool in the round data
    if prize_pool is None:
        existing_prize_pool = round_data[round_id].get('prize_pool')
        if existing_prize_pool is None:
            print(f"Error: No prize pool defined for {round_id}. Please provide a prize pool.")
            sys.exit(1)
        prize_pool = existing_prize_pool
    else:
        # Store the prize pool in the round data for future use
        round_data[round_id]['prize_pool'] = prize_pool
    
    # Get target image path and participants
    target_image = Path(f"rounds/{round_data[round_id]['target_image']}")
    participants = round_data[round_id]['participants']
    
    if not target_image.exists():
        print(f"Error: Target image {target_image} does not exist")
        sys.exit(1)
    
    if not participants:
        print(f"No participants found for {round_id}")
        sys.exit(1)
    
    # Verify commitments if requested
    if verify_commitments:
        print(f"\nVerifying commitments for {round_id}...")
        all_valid = verify_round_commitments(round_id)
        
        if not all_valid and not input("\nSome commitments are invalid. Continue anyway? (y/n): ").lower().startswith('y'):
            print("Aborted.")
            sys.exit(1)
    
    # Reload the data after verification (as it may have updated the valid fields)
    with open(guesses_file, 'r') as f:
        round_data = json.load(f)
    participants = round_data[round_id]['participants']
    
    # Get the appropriate validator for this round
    validator = get_validator_for_round(round_id)
    
    # Get image features
    image_features = validator.embedder.get_image_embedding(target_image)
    
    # Process each participant's guess
    valid_participants = []
    for participant in participants:
        if participant['valid']:
            guess = participant['guess']
            score = validator.calculate_adjusted_score(image_features, guess)
            participant['score'] = round(float(score), 6)
            valid_participants.append((participant, score))
    
    # Sort by score (highest first)
    valid_participants.sort(key=lambda x: x[1], reverse=True)
    
    # Calculate payouts
    if valid_participants:
        # Format for payout calculation
        ranked_guesses = [(p[0]['guess'], p[1]) for p in valid_participants]
        payouts = calculate_payouts(ranked_guesses, prize_pool)
        
        # Update participants with payout information
        for (participant, _), payout in zip(valid_participants, payouts):
            participant['payout'] = round(float(payout), 6)
    
        # Update round data
        round_data[round_id]['total_payout'] = prize_pool
        
        # Save results back to file
        if save_to_file:
            with open(guesses_file, 'w') as f:
                json.dump(round_data, f, indent=4)
    
        # Display results
        print(f"\nResults for {round_id}:")
        print("-" * 50)
        for i, ((participant, score), payout) in enumerate(zip(valid_participants, payouts), 1):
            print(f"{i}. {participant['username']}")
            print(f"   Guess: \"{participant['guess']}\"")
            print(f"   Score: {score:.6f}")
            print(f"   Payout: {payout:.6f}")
            print()
        
        print(f"Total prize pool: {prize_pool:.6f}")
        print(f"Total payout: {sum(payouts):.6f}")
    else:
        print(f"No valid participants found for {round_id}")
    
    return round_data

def process_all_rounds(save_to_file=True, verify_commitments=True):
    """Process payouts for all rounds that have participants but no payouts.
    
    Args:
        save_to_file: Whether to save results back to guesses.json
        verify_commitments: Whether to verify commitments before processing
        
    Returns:
        Dictionary with all round data including calculated scores and payouts
    """
    # Load guesses
    guesses_file = Path("rounds/guesses.json")
    if not guesses_file.exists():
        print(f"Error: {guesses_file} does not exist")
        sys.exit(1)
        
    with open(guesses_file, 'r') as f:
        round_data = json.load(f)
    
    # Process each round
    processed_rounds = []
    for round_id in round_data:
        # Skip rounds without participants
        if not round_data[round_id]['participants']:
            continue
            
        # Skip rounds that already have payouts calculated
        if round_data[round_id].get('total_payout') is not None:
            print(f"Skipping {round_id} - payouts already calculated")
            continue
            
        # Check if prize pool is defined for this round
        prize_pool = round_data[round_id].get('prize_pool')
        if prize_pool is None:
            print(f"Skipping {round_id} - no prize pool defined")
            continue
            
        print(f"Processing {round_id}...")
        process_round_payouts(round_id, prize_pool, save_to_file, verify_commitments)
        processed_rounds.append(round_id)
    
    if not processed_rounds:
        print("No rounds to process")
    
    return round_data

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Process payouts for Cliptions rounds")
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--round', type=str, help="Process a specific round (e.g., round1)")
    group.add_argument('--all', action='store_true', help="Process all rounds that need payouts")
    parser.add_argument('--prize-pool', type=float, help="Prize pool amount (only required for new rounds)")
    parser.add_argument('--no-save', action='store_true', help="Don't save results back to guesses.json")
    parser.add_argument('--skip-verify', action='store_true', help="Skip commitment verification")
    
    args = parser.parse_args()
    
    if args.round:
        process_round_payouts(args.round, args.prize_pool, not args.no_save, not args.skip_verify)
    else:  # args.all
        process_all_rounds(not args.no_save, not args.skip_verify) 