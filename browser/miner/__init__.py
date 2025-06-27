"""
Cliptions Miner Automation Modules

This package contains modules for automating miner participation in Cliptions prediction rounds.
Miners use these modules to submit commitments and reveals in response to validator announcements.
"""

from .submit_commitment import CommitmentSubmissionTask, CommitmentSubmissionData, CommitmentSubmissionResult

__all__ = [
    'CommitmentSubmissionTask',
    'CommitmentSubmissionData', 
    'CommitmentSubmissionResult'
] 