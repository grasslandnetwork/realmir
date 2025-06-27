"""
Schema Consistency Test

This test acts as a "consistency lock" between the Pydantic models defined in Python
and the corresponding data structures (structs) defined in Rust.

It works by:
1. Creating instances of the Pydantic models with sample data.
2. Serializing them to Python dictionaries.
3. Passing these dictionaries to special test functions in the Rust core library.
4. The Rust functions attempt to deserialize the dictionaries into Rust structs.

If the Rust deserialization succeeds, the test passes. If it fails (due to a
mismatch in fields, types, etc.), it will raise a Python exception, and the
test will fail.

This ensures that our Python and Rust data models cannot drift apart.
"""

import sys
from pathlib import Path

# Add the project root to Python path for imports
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

import pytest
from datetime import datetime
from browser.data_models import Commitment, Round

# Attempt to import the Rust core library. If it fails, skip these tests.
try:
    from cliptions_core import test_deserialize_commitment, test_deserialize_round
except ImportError:
    # Set a flag to skip all tests in this file
    pytest.skip("Could not import cliptions_core. Run 'maturin develop' to build the Rust library.", allow_module_level=True)


def test_commitment_schema_consistency():
    """
    Tests that the Pydantic Commitment model is consistent with the Rust Commitment struct.
    """
    # 1. Create a Pydantic Commitment instance with sample data
    pydantic_commitment = Commitment(
        username="@test_miner",
        commitment_hash="0x" + "a" * 64,
        wallet_address="5Co2unDtZKZDzYNZHT2fUMkEnpVWnassfbuabvZmGTrYKgtD",
        tweet_url="https://x.com/realmir_testnet/status/12345",
        timestamp=datetime.now()
    )

    # 2. Convert to a dictionary, using JSON-compatible types
    commitment_dict = pydantic_commitment.model_dump(mode="json")

    # 3. Pass the dictionary to the Rust test function
    try:
        test_deserialize_commitment(commitment_dict)
    except Exception as e:
        pytest.fail(f"Rust failed to deserialize Pydantic Commitment model: {e}")


def test_round_schema_consistency():
    """
    Tests that the Pydantic Round model is consistent with the Rust Round struct.
    """
    # 1. Create Pydantic instances
    pydantic_commitment = Commitment(
        username="@test_miner",
        commitment_hash="0x" + "a" * 64,
        wallet_address="5Co2unDtZKZDzYNZHT2fUMkEnpVWnassfbuabvZmGTrYKgtD",
        tweet_url="https://x.com/realmir_testnet/status/12345",
        timestamp=datetime.now()
    )
    
    pydantic_round = Round(
        round_id="test_round_001",
        announcement_url="https://x.com/realmir_testnet/status/12344",
        livestream_url="https://youtube.com/live/some_id",
        entry_fee=0.001,
        commitment_deadline=datetime.now(),
        reveal_deadline=datetime.now(),
        commitments=[pydantic_commitment]
    )

    # 2. Convert to a dictionary
    round_dict = pydantic_round.model_dump(mode="json")
    
    # 3. Pass to the Rust test function
    try:
        test_deserialize_round(round_dict)
    except Exception as e:
        pytest.fail(f"Rust failed to deserialize Pydantic Round model: {e}")

def test_round_with_empty_commitments():
    """
    Tests that a Round with no commitments can be deserialized correctly.
    This validates the `#[serde(default)]` attribute on the Rust side.
    """
    pydantic_round = Round(
        round_id="test_round_002",
        announcement_url="https://x.com/realmir_testnet/status/12346",
        livestream_url="https://youtube.com/live/some_id_2",
        entry_fee=0.001,
        commitment_deadline=datetime.now(),
        reveal_deadline=datetime.now(),
        commitments=[]  # Empty list
    )

    round_dict = pydantic_round.model_dump(mode="json")
    
    try:
        test_deserialize_round(round_dict)
    except Exception as e:
        pytest.fail(f"Rust failed to deserialize Round with empty commitments: {e}") 