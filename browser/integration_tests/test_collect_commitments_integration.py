#!/usr/bin/env python3
"""
Integration Test for CollectCommitmentsTask

This test performs a full end-to-end integration test of the commitment collection
functionality using real browser automation and actual Twitter/X pages.
"""

import asyncio
import os
import sys
import pathlib
import pytest

# Add project root to the Python path to allow absolute imports
project_root = pathlib.Path(__file__).parent.parent.parent
sys.path.insert(0, str(project_root))

from browser.validator.collect_commitments import CollectCommitmentsTask


@pytest.mark.integration
@pytest.mark.asyncio
async def test_collect_commitments_integration():
    """
    Integration test for collecting commitments from a real Twitter announcement.
    
    This test uses the actual Round 2 announcement which has known commitment replies
    from davidynamic and track_data_.
    """
    # Round 2 announcement URL with multiple commitment replies
    announcement_url = "https://x.com/realmir_testnet/status/1907159517013422578"
    
    print(f"\n🧪 Integration Test: Collecting commitments from {announcement_url}")
    
    # Initialize the commitment collector
    collector = CollectCommitmentsTask()
    
    try:
        # Execute the collection
        print("🚀 Starting commitment collection...")
        results = await collector.execute(announcement_url)
        
        # Validate results
        print(f"\n=== INTEGRATION TEST RESULTS ===")
        print(f"Success: {results.success}")
        print(f"Total commitments found: {results.total_commitments_found}")
        
        # Assert basic success criteria
        assert results.success is True, "Collection should succeed"
        assert results.total_commitments_found >= 2, "Should find at least 2 commitments (davidynamic and track_data_)"
        assert len(results.commitments) >= 2, "Should have at least 2 commitment objects"
        
        # Validate commitment data structure
        for i, commitment in enumerate(results.commitments, 1):
            print(f"\n{i}. {commitment.username}")
            print(f"   Commitment Hash: {commitment.commitment_hash}")
            print(f"   Wallet Address: {commitment.wallet_address}")
            print(f"   Tweet URL: {commitment.tweet_url}")
            
            # Assert required fields are present and valid
            assert commitment.username.startswith("@"), f"Username should start with @: {commitment.username}"
            assert len(commitment.commitment_hash) > 32, f"Commitment hash should be substantial: {commitment.commitment_hash}"
            assert len(commitment.wallet_address) > 20, f"Wallet address should be substantial: {commitment.wallet_address}"
        
        # Check for expected participants (based on rounds/guesses.json)
        usernames = [c.username for c in results.commitments]
        assert "@davidynamic" in usernames, "Should find davidynamic's commitment"
        assert "@track_data_" in usernames, "Should find track_data_'s commitment"
        
        print(f"\n✅ Integration test passed! Found {results.total_commitments_found} valid commitments.")
        
    except Exception as e:
        print(f"❌ Integration test failed: {str(e)}")
        raise
    
    finally:
        # Clean up browser resources
        await collector.cleanup()
        print("🧹 Browser resources cleaned up")


@pytest.mark.integration
@pytest.mark.asyncio
async def test_collect_commitments_empty_announcement():
    """
    Integration test for handling announcements with no commitment replies.
    """
    # Use a different tweet URL that likely has no commitment replies
    announcement_url = "https://x.com/realmir_testnet/status/1907171976684187882"  # This is a reveal URL
    
    print(f"\n🧪 Integration Test: Testing empty announcement handling")
    
    collector = CollectCommitmentsTask()
    
    try:
        results = await collector.execute(announcement_url)
        
        # Should succeed but find no commitments
        assert results.success is True, "Collection should succeed even with no commitments"
        assert results.total_commitments_found == 0, "Should find no commitments in reveal tweet"
        assert len(results.commitments) == 0, "Should have empty commitments list"
        
        print("✅ Empty announcement test passed!")
        
    finally:
        await collector.cleanup()


if __name__ == "__main__":
    """
    Run the integration test directly (outside of pytest)
    """
    print("=== Cliptions Commitment Collection Integration Test ===")
    print("This will perform a full end-to-end test using real browser automation.")
    print()
    
    # Check if we should run in test mode
    test_mode = os.environ.get('TEST_MODE', 'false').lower() == 'true'
    
    if test_mode:
        print("🧪 Running in TEST MODE - will not perform actual extraction")
        print("Set TEST_MODE=false to run real integration test")
        exit(0)
    
    # Run the main integration test
    asyncio.run(test_collect_commitments_integration()) 