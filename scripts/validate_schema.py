#!/usr/bin/env python3
"""Validate shared AuxScreen protocol fixtures against the v1 schema."""

import json
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "protocol" / "schema" / "control-v1.schema.json"
FIXTURES = ROOT / "protocol" / "fixtures"


def main() -> None:
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    fixtures = sorted(FIXTURES.glob("*.json"))
    if not fixtures:
        raise SystemExit("no protocol fixtures found")
    for fixture in fixtures:
        validator.validate(json.loads(fixture.read_text(encoding="utf-8")))
        print(f"valid: {fixture.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
