import json
from pathlib import Path
from pydantic import TypeAdapter
from tandem_client.types import EngineEvent

CONTRACT_PATH = Path(__file__).parent.parent.parent.parent / "contracts" / "events.json"

_engine_event_adapter = TypeAdapter(EngineEvent)

def test_events_contract():
    assert CONTRACT_PATH.exists(), f"Could not find events.json at {CONTRACT_PATH}"
    
    events_contract = json.loads(CONTRACT_PATH.read_text())
    assert len(events_contract) > 0

    for event_def in events_contract:
        event_type = event_def["type"]
        required_fields = event_def["required"]
        
        # Mock tolerant wire format payload
        mock_wire_payload = {
            "type": event_type,
            "timestamp": "2024-01-01T00:00:00Z",
            "properties": {"custom": "data"}
        }
        
        # Populate varying wire forms
        if "sessionId" in required_fields:
            mock_wire_payload["sessionID"] = "s_123"
        if "runId" in required_fields:
            mock_wire_payload["run_id"] = "r_456"

        # Validate with TypeAdapter
        event = _engine_event_adapter.validate_python(mock_wire_payload)

        # Assert Canonical properties
        assert event.type == event_type
        assert event.properties == {"custom": "data"}
        assert event.timestamp == "2024-01-01T00:00:00Z"
        
        if "sessionId" in required_fields:
            assert event.session_id == "s_123"
        if "runId" in required_fields:
            assert event.run_id == "r_456"

        print(f"Passed: {event_type}")
