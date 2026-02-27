from typing import Any

class TandemError(Exception):
    """Base exception for all Tandem SDK errors."""
    pass

class TandemValidationError(TandemError):
    """Raised when the engine response fails Pydantic schema validation."""
    def __init__(self, endpoint: str, status: int, issues: Any, raw_snippet: str) -> None:
        super().__init__(f"Tandem API Validation Error [{status}] at {endpoint}: {len(issues)} issues found.")
        self.endpoint = endpoint
        self.status = status
        self.issues = issues
        self.raw_snippet = raw_snippet
