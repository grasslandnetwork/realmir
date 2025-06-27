"""
Commitment Submission Module for Cliptions Miners

This module implements the TwitterPostingInterface to submit commitment hashes as replies
to validator round announcement tweets. Miners use this to participate in prediction rounds
by submitting their cryptographic commitments along with their wallet addresses.
"""

import logging
from datetime import datetime
from typing import Dict, Any, Optional
from pydantic import BaseModel, Field

try:
    # Try relative imports first (when used as part of package)
    from ..core.interfaces import TwitterPostingInterface
    from ..core.base_task import BaseTwitterTask
    from ...core.generate_commitment import generate_commitment
except ImportError:
    # Fall back to direct imports (when used as standalone via sys.path tweaks)
    from browser.core.interfaces import TwitterPostingInterface
    from browser.core.base_task import BaseTwitterTask
    from core.generate_commitment import generate_commitment


class CommitmentSubmissionData(BaseModel):
    """Data structure for commitment submission content"""
    prediction: str = Field(..., description="The plaintext prediction")
    salt: str = Field(..., description="Salt used to generate commitment hash")
    wallet_address: str = Field(..., description="Miner's wallet address for payouts")
    reply_to_url: str = Field(..., description="URL of the round announcement tweet to reply to")
    commitment_hash: Optional[str] = Field(None, description="Pre-computed commitment hash (optional)")


class CommitmentSubmissionResult(BaseModel):
    """Result from posting a commitment submission"""
    success: bool = Field(..., description="Whether the commitment was submitted successfully")
    tweet_url: Optional[str] = Field(None, description="URL of the posted commitment tweet")
    tweet_id: Optional[str] = Field(None, description="ID of the posted commitment tweet")
    commitment_hash: str = Field(..., description="The commitment hash that was submitted")
    wallet_address: str = Field(..., description="The wallet address submitted")
    timestamp: datetime = Field(default_factory=datetime.now, description="When the commitment was submitted")
    error_message: Optional[str] = Field(None, description="Error message if submission failed")


class CommitmentSubmissionTask(BaseTwitterTask):
    """
    Task for submitting commitment hashes to Twitter as replies to round announcements.
    
    This task implements the TwitterPostingInterface to handle a miner's
    participation in a prediction round by submitting their commitment.
    """
    
    def __init__(self, config_path: Optional[str] = None):
        super().__init__(config_path)
        self.logger = logging.getLogger(__name__)
    
    async def execute(self, **kwargs) -> CommitmentSubmissionResult:
        """
        Execute the commitment submission posting task.
        
        Args:
            **kwargs: Should contain CommitmentSubmissionData fields or a 'data' key
                     with CommitmentSubmissionData instance
        
        Returns:
            CommitmentSubmissionResult: Result of the commitment submission
        """
        try:
            # Use the base class execute method which includes cleanup
            result = await super().execute(**kwargs)
            return result
        except Exception as e:
            self.logger.error(f"Failed to submit commitment: {str(e)}")
            return CommitmentSubmissionResult(
                success=False,
                commitment_hash="",
                wallet_address=kwargs.get('wallet_address', 'unknown'),
                error_message=str(e)
            )
    
    async def _execute_task(self, **kwargs) -> CommitmentSubmissionResult:
        """
        Internal task execution method called by the base class.
        
        Args:
            **kwargs: Should contain CommitmentSubmissionData fields or a 'data' key
                     with CommitmentSubmissionData instance
        
        Returns:
            CommitmentSubmissionResult: Result of the commitment submission
        """
        # Parse input data
        if 'data' in kwargs:
            submission_data = kwargs['data']
            if not isinstance(submission_data, CommitmentSubmissionData):
                submission_data = CommitmentSubmissionData(**submission_data)
        else:
            submission_data = CommitmentSubmissionData(**kwargs)
        
        self.logger.info(f"Starting commitment submission for wallet {submission_data.wallet_address}")
        
        # Generate commitment hash if not provided
        if not submission_data.commitment_hash:
            submission_data.commitment_hash = generate_commitment(
                submission_data.prediction, 
                submission_data.salt
            )
        
        # Format the commitment content
        content = self.format_content(submission_data.dict())
        
        # Post the commitment as a reply
        result = await self.post_content(content, reply_to_url=submission_data.reply_to_url)
        
        return CommitmentSubmissionResult(
            success=True,
            tweet_url=result.get('tweet_url'),
            tweet_id=result.get('tweet_id'),
            commitment_hash=submission_data.commitment_hash,
            wallet_address=submission_data.wallet_address,
            timestamp=datetime.now()
        )
    
    def format_content(self, data: Dict[str, Any]) -> str:
        """
        Format the commitment submission content for Twitter.
        
        Args:
            data: Commitment submission data
            
        Returns:
            Formatted tweet content
        """
        commitment_hash = data.get('commitment_hash', '')
        wallet_address = data.get('wallet_address', '')
        
        content_parts = [
            f"Commit: {commitment_hash}",
            f"Wallet: {wallet_address}"
        ]
        
        return "\n".join(content_parts)
    
    async def post_content(self, content: str, **kwargs) -> Dict[str, Any]:
        """
        Post commitment content to Twitter as a reply using browser automation.
        
        Args:
            content: The formatted commitment content
            **kwargs: Additional parameters including reply_to_url
            
        Returns:
            Dictionary with posting results
        """
        reply_to_url = kwargs.get('reply_to_url')
        if not reply_to_url:
            raise ValueError("reply_to_url is required for commitment submissions")
        
        try:
            # 1. Define initial actions to navigate to the tweet to reply to
            initial_actions = [
                {'go_to_url': {'url': reply_to_url}},
            ]

            # 2. Define a specific task for replying to the tweet
            task_description = f"""
            You are on a Twitter/X tweet page. Your task is to reply to this tweet with the following content:

            ---
            {content}
            ---

            Follow these steps precisely:
            1. Locate the reply button/area. This is typically a text input with placeholder like "Post your reply" or "Tweet your reply".
            2. Click on the reply input area to focus it.
            3. Use the `send_keys` action to type the exact content provided above into the reply area.
            4. Locate the 'Reply' or 'Post' button to submit the reply.
            5. Click the reply/post button to publish the reply.
            6. Wait for confirmation that the reply was sent, then use the `done` action.
            """
            
            # 3. Set up the browser agent
            agent = await self.setup_agent(
                task=task_description,
                initial_actions=initial_actions,
            )
            
            # Run the agent
            print("🤖 Agent starting task: Posting commitment reply...")
            result = await agent.run(max_steps=10)
            
            print(f"Agent finished with result: {result}")
            
            # Extract tweet URL and ID from the final result or agent history
            tweet_url = "https://twitter.com/placeholder_reply_url"  # Placeholder
            tweet_id = "placeholder_reply_id" # Placeholder
            
            if hasattr(result, 'history'):
                # You can add logic here to parse history for the final URL if needed
                pass

            return {
                "success": True,
                "tweet_url": tweet_url,
                "tweet_id": tweet_id,
                "message": "Successfully posted commitment reply"
            }
            
        except Exception as e:
            logging.error(f"Failed to post commitment reply to Twitter: {e}")
            import traceback
            traceback.print_exc()
            return {
                "success": False,
                "message": str(e)
            }
    
    def validate_output(self, result: Any) -> CommitmentSubmissionResult:
        """
        Validate that the commitment was submitted successfully.
        
        Args:
            result: The result to validate
            
        Returns:
            Validated CommitmentSubmissionResult
        """
        if isinstance(result, CommitmentSubmissionResult):
            return result
        
        # If it's not already a CommitmentSubmissionResult, something went wrong
        return CommitmentSubmissionResult(
            success=False,
            commitment_hash="",
            wallet_address="unknown",
            error_message="Invalid result type returned from task execution"
        )


# Utility functions for creating commitment submission data

def create_commitment_submission(
    prediction: str,
    salt: str,
    wallet_address: str,
    reply_to_url: str
) -> CommitmentSubmissionData:
    """
    Create a commitment submission with the required parameters.
    
    Args:
        prediction: The plaintext prediction to commit
        salt: Salt value for the commitment hash
        wallet_address: Miner's wallet address for payouts
        reply_to_url: URL of the round announcement tweet to reply to
        
    Returns:
        CommitmentSubmissionData instance
    """
    return CommitmentSubmissionData(
        prediction=prediction,
        salt=salt,
        wallet_address=wallet_address,
        reply_to_url=reply_to_url
    )


def create_precomputed_commitment_submission(
    commitment_hash: str,
    wallet_address: str,
    reply_to_url: str,
    prediction: str = "",
    salt: str = ""
) -> CommitmentSubmissionData:
    """
    Create a commitment submission with a pre-computed hash.
    
    Args:
        commitment_hash: Pre-computed commitment hash
        wallet_address: Miner's wallet address for payouts
        reply_to_url: URL of the round announcement tweet to reply to
        prediction: Original prediction (optional, for reference)
        salt: Original salt (optional, for reference)
        
    Returns:
        CommitmentSubmissionData instance
    """
    return CommitmentSubmissionData(
        prediction=prediction,
        salt=salt,
        wallet_address=wallet_address,
        reply_to_url=reply_to_url,
        commitment_hash=commitment_hash
    ) 