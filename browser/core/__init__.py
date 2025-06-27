"""
Cliptions Twitter Automation Core Module

This package provides the foundational interfaces and base classes for 
the modular Twitter automation system.
"""

from .interfaces import (
    TwitterTask,
    TwitterExtractionInterface, 
    TwitterPostingInterface,
    TwitterTaskError,
    ExtractionError,
    PostingError,
    ValidationError
)

from .base_task import BaseTwitterTask
from .cost_tracker import BrowserUseCostTracker, create_cost_tracker_from_config

__all__ = [
    # Interfaces
    'TwitterTask',
    'TwitterExtractionInterface',
    'TwitterPostingInterface',
    
    # Base Implementation
    'BaseTwitterTask',
    
    # Cost Tracking
    'BrowserUseCostTracker',
    'create_cost_tracker_from_config',
    
    # Exceptions
    'TwitterTaskError',
    'ExtractionError', 
    'PostingError',
    'ValidationError'
] 