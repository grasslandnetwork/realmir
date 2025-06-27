"""
Validator modules for Cliptions Twitter automation.

This package contains modules that implement the Validator side of the Cliptions prediction game:
- Round announcements
- Commitment collection  
- Entry fee assignment
- Target frame publication
- Reveal collection
- Payment verification
- Results publication
"""

# Note: Expose RoundAnnouncementTask for easy import
from .announce_round import RoundAnnouncementTask

__all__ = ['RoundAnnouncementTask'] 