"""
Pydantic Data Models for Cliptions

This module defines the Pydantic models that mirror the core data structures
defined in the Rust `src/models.rs`. These models are used for data validation,
serialization, and ensuring a consistent data schema between the Python and Rust parts
of the application.
"""
from datetime import datetime
from typing import List
from pydantic import BaseModel, Field

class Commitment(BaseModel):
    """
    Represents a single parsed commitment from a tweet reply.
    Mirrors the Rust `Commitment` struct.
    """
    username: str = Field(..., description="The Twitter username of the miner who submitted the commitment.")
    commitment_hash: str = Field(..., description="The SHA-256 commitment hash.")
    wallet_address: str = Field(..., description="The miner's wallet address for payouts.")
    tweet_url: str = Field(..., description="The URL of the reply tweet containing the commitment.")
    timestamp: datetime = Field(..., description="The timestamp when the reply was posted.")

class Round(BaseModel):
    """
    Represents a full prediction round.
    Mirrors the Rust `Round` struct.
    """
    round_id: str = Field(..., description="Unique identifier for the round.")
    announcement_url: str = Field(..., description="URL of the announcement tweet that was processed.")
    livestream_url: str = Field(..., description="URL of the livestream players are predicting.")
    entry_fee: float = Field(..., description="Entry fee in TAO.")
    commitment_deadline: datetime = Field(..., description="Deadline for commitment submissions.")
    reveal_deadline: datetime = Field(..., description="Deadline for reveal submissions.")
    commitments: List[Commitment] = Field(default_factory=list, description="A list of all collected commitments.") 