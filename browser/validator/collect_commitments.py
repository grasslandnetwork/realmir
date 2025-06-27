"""
Commitment Collection Module for Cliptions Validators

This module is responsible for extracting all miner commitment submissions
from the replies to a validator's round announcement tweet.
"""

import asyncio
import json
import logging
from datetime import datetime
from typing import List, Optional, Any
from pydantic import BaseModel, Field

from ..core.base_task import BaseTwitterTask
from ..core.interfaces import ExtractionError


class CommitmentData(BaseModel):
    """
    Represents a single parsed commitment from a tweet reply.
    """
    username: str = Field(..., description="The Twitter username of the miner who submitted the commitment.")
    commitment_hash: str = Field(..., description="The SHA-256 commitment hash.")
    wallet_address: str = Field(..., description="The miner's wallet address for payouts.")
    tweet_url: str = Field(..., description="The URL of the reply tweet containing the commitment.")
    timestamp: datetime = Field(..., description="The timestamp when the reply was posted.")


class CommitmentCollectionResult(BaseModel):
    """
    The result of the commitment collection task, containing all found commitments.
    """
    success: bool = Field(..., description="Indicates whether the extraction was successful.")
    commitments: List[CommitmentData] = Field(default_factory=list, description="A list of all collected commitments.")
    announcement_url: str = Field(..., description="The URL of the announcement tweet that was processed.")
    total_commitments_found: int = Field(default=0, description="Total number of commitments extracted.")
    error_message: Optional[str] = Field(None, description="An error message if the task failed.")


class CollectCommitmentsTask(BaseTwitterTask):
    """
    A task to collect commitment submissions from a Twitter thread using Browser Use.
    
    This task extracts replies to a round announcement tweet and parses them to find
    miner commitments in the format:
    - Commit: <hash>
    - Wallet: <address>
    
    Follows SOLID principles:
    - Single Responsibility: Only handles commitment extraction
    - Open/Closed: Extensible through inheritance
    - Liskov Substitution: Can be used anywhere BaseTwitterTask is expected
    - Interface Segregation: Clean, focused interface
    - Dependency Inversion: Depends on base abstractions
    """
    
    def __init__(self, config_file_path: Optional[str] = None):
        """Initialize the commitment collector with configuration and cost tracking"""
        super().__init__(config_file_path=config_file_path)
        self.logger = logging.getLogger(__name__)

    async def execute(self, **kwargs) -> CommitmentCollectionResult:
        """
        Execute the commitment collection task by extracting and parsing replies.

        Args:
            **kwargs: Should contain 'announcement_url' key

        Returns:
            A CommitmentCollectionResult containing the collected commitment data.
        """
        announcement_url = kwargs.get('announcement_url')
        if not announcement_url:
            raise ExtractionError("announcement_url is required for commitment collection")
        
        try:
            # Use the base class execute method which includes cleanup
            result = await super().execute(**kwargs)
            return result
        except Exception as e:
            self.logger.error(f"Failed to collect commitments: {str(e)}")
            return CommitmentCollectionResult(
                success=False,
                commitments=[],
                announcement_url=announcement_url,
                total_commitments_found=0,
                error_message=str(e)
            )

    async def _execute_task(self, **kwargs) -> CommitmentCollectionResult:
        """
        Internal task execution method called by the base class.
        
        Args:
            **kwargs: Should contain 'announcement_url' key
        
        Returns:
            CommitmentCollectionResult: Result of the commitment collection
        """
        announcement_url = kwargs['announcement_url']
        
        self.logger.info(f"Starting commitment collection for announcement: {announcement_url}")
        print(f"Collecting commitments from: {announcement_url}")
        
        # Define initial actions to run without LLM (faster and cheaper)
        initial_actions = [
            {'open_tab': {'url': announcement_url}},  # Navigate directly to the announcement tweet
        ]
        
        # Define the commitment extraction task (simplified to prevent loops)
        task = f"""
        You are on a Twitter/X page showing an announcement tweet. Your ONLY job is to find and extract commitment replies.

        STRICT RULES - FOLLOW EXACTLY:
        1. NEVER click on usernames, profile pictures, or tweet links
        2. NEVER click "Reply", "Retweet", "Like", or any interaction buttons  
        3. ONLY use scroll_down to see more content
        4. ONLY click "Show more replies" or "Show probable spam" buttons if you see them
        5. When you have scrolled enough and seen all replies, immediately return the JSON result

        WHAT TO LOOK FOR:
        Find replies that contain BOTH lines:
        - "Commit: [some hash]"
        - "Wallet: [some address]"

        PROCESS:
        1. Look at what's currently visible
        2. Scroll down 3-5 times to load more replies
        3. If you see "Show more replies" or "Show probable spam", click it once
        4. Scroll down 2-3 more times
        5. Extract all commitment data you can see
        6. Return the JSON result immediately

        RETURN FORMAT:
        {{
            "announcement_url": "{announcement_url}",
            "success": true,
            "total_commitments_found": <number>,
            "commitments": [
                {{
                    "username": "@username",
                    "commitment_hash": "the_hash_value",
                    "wallet_address": "the_wallet_address",
                    "tweet_url": "",
                    "timestamp": "2024-01-01T00:00:00Z",
                    "was_spam_flagged": false
                }}
            ]
        }}

        IMPORTANT: Do this quickly and efficiently. Don't overthink it. Just scroll, find commitments, return JSON.
        """
        
        # Setup and run the agent using base class method
        agent = await self.setup_agent(
            task=task,
            initial_actions=initial_actions,
            use_vision=False  # Disable vision for better performance
        )
        
        # Run the extraction
        print("Starting commitment extraction with initial navigation...")
        max_steps = self.get_max_steps()
        result = await agent.run(max_steps=max_steps)
        
        # Parse the extraction result
        return await self._parse_extraction_result(result, announcement_url)

    async def _parse_extraction_result(self, result, announcement_url: str) -> CommitmentCollectionResult:
        """Parse the browser extraction result into structured commitment data"""
        
        # Handle different result types (adapted from get_twitter_replies.py pattern)
        result_data = None
        
        if isinstance(result, str):
            try:
                # Try to parse as JSON if it looks like JSON
                if result.strip().startswith('{') and result.strip().endswith('}'):
                    result_data = json.loads(result)
            except json.JSONDecodeError:
                pass
                
        elif hasattr(result, 'final_result'):
            final_result = result.final_result()
            if final_result:
                try:
                    if isinstance(final_result, str) and final_result.strip().startswith('{'):
                        result_data = json.loads(final_result)
                except json.JSONDecodeError:
                    pass
        
        # If we successfully parsed JSON data, process it
        if result_data and isinstance(result_data, dict):
            return await self._process_parsed_data(result_data, announcement_url)
        
        # Fallback: try to extract from result string using regex
        return await self._fallback_text_parsing(str(result), announcement_url)

    async def _process_parsed_data(self, data: dict, announcement_url: str) -> CommitmentCollectionResult:
        """Process successfully parsed JSON data into CommitmentCollectionResult"""
        
        commitments = []
        
        # Extract commitments from the parsed data
        raw_commitments = data.get('commitments', [])
        
        for raw_commitment in raw_commitments:
            try:
                # Validate that both Commit and Wallet are present
                username = raw_commitment.get('username', '').strip()
                commitment_hash = raw_commitment.get('commitment_hash', '').strip()
                wallet_address = raw_commitment.get('wallet_address', '').strip()
                
                if not username or not commitment_hash or not wallet_address:
                    print(f"Skipping incomplete commitment: {raw_commitment}")
                    continue
                
                # Create CommitmentData object
                commitment = CommitmentData(
                    username=username,
                    commitment_hash=commitment_hash,
                    wallet_address=wallet_address,
                    tweet_url=raw_commitment.get('tweet_url', ''),
                    timestamp=datetime.fromisoformat(raw_commitment.get('timestamp', datetime.now().isoformat()))
                )
                
                commitments.append(commitment)
                print(f"✅ Parsed commitment from {username}: {commitment_hash[:16]}...")
                
            except Exception as e:
                print(f"Error parsing commitment {raw_commitment}: {e}")
                continue
        
        return CommitmentCollectionResult(
            success=True,
            commitments=commitments,
            announcement_url=announcement_url,
            total_commitments_found=len(commitments)
        )

    async def _fallback_text_parsing(self, result_text: str, announcement_url: str) -> CommitmentCollectionResult:
        """Fallback method to extract commitments from raw text using regex patterns"""
        
        print("Using fallback text parsing for commitment extraction...")
        
        commitments = []
        
        # Look for patterns like:
        # @username
        # Commit: hash
        # Wallet: address
        
        import re
        
        # Pattern to find commitment blocks
        pattern = r'@(\w+).*?Commit:\s*([a-fA-F0-9]+).*?Wallet:\s*([^\s\n]+)'
        matches = re.findall(pattern, result_text, re.DOTALL | re.IGNORECASE)
        
        for match in matches:
            try:
                username, commitment_hash, wallet_address = match
                
                commitment = CommitmentData(
                    username=f"@{username}",
                    commitment_hash=commitment_hash.strip(),
                    wallet_address=wallet_address.strip(),
                    tweet_url="",  # Not available in fallback parsing
                    timestamp=datetime.now()
                )
                
                commitments.append(commitment)
                print(f"✅ Fallback parsed commitment from @{username}: {commitment_hash[:16]}...")
                
            except Exception as e:
                print(f"Error in fallback parsing: {e}")
                continue
        
        return CommitmentCollectionResult(
            success=len(commitments) > 0,
            commitments=commitments,
            announcement_url=announcement_url,
            total_commitments_found=len(commitments),
            error_message="Used fallback text parsing" if len(commitments) > 0 else "No commitments found in fallback parsing"
        )

    def validate_output(self, result: Any) -> CommitmentCollectionResult:
        """
        Validate that the commitment collection result is properly formatted.
        
        Args:
            result: Raw result from task execution
            
        Returns:
            Validated CommitmentCollectionResult
        """
        if isinstance(result, CommitmentCollectionResult):
            return result
        
        # If result is not already a CommitmentCollectionResult, try to convert
        if isinstance(result, dict):
            try:
                return CommitmentCollectionResult(**result)
            except Exception as e:
                raise ExtractionError(f"Failed to validate commitment collection result: {e}")
        
        raise ExtractionError(f"Invalid result type for commitment collection: {type(result)}")

    async def save_results(self, results: CommitmentCollectionResult, output_file: str = "commitments.json"):
        """Save the commitment collection results to a JSON file"""
        
        with open(output_file, 'w') as f:
            json.dump(results.model_dump(), f, indent=2, default=str)
        
        print(f"💾 Results saved to {output_file}")

    # cleanup method is handled by the base class 