#!/usr/bin/env python3
"""Migrate pedalboards saved before the dB level migration.

The input and output are JSON files in the format written by rs-pedalboard.
The converter does not modify client settings.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
from pathlib import Path
from typing import Any


MINIMUM_DB = -70.0
MAXIMUM_DB = 40.0

# These pedals stored gain/level as a linear multiplier before the migration.
LINEAR_DB_PARAMETERS = {
    "Compressor": {"Level"},
    "Distortion": {"Level"},
    "Fuzz": {"Level"},
    "ImpulseResponse": {"Level"},
    "Nam": {"Gain", "Level"},
    "Overdrive": {"Level"},
    "Volume": {"Volume"},
}


def linear_to_db(value: float) -> float:
    """Convert a linear amplitude multiplier to the dB range used by pedals."""
    if value <= 0.0:
        return MINIMUM_DB
    return max(MINIMUM_DB, min(MAXIMUM_DB, 20.0 * math.log10(value)))


def _float_value(parameter: dict[str, Any]) -> float | None:
    value = parameter.get("value")
    if not isinstance(value, dict) or len(value) != 1 or "Float" not in value:
        return None
    raw_value = value["Float"]
    return raw_value if isinstance(raw_value, (int, float)) and not isinstance(raw_value, bool) else None


def _set_float(parameter: dict[str, Any], value: float) -> None:
    parameter["value"] = {"Float": value}
    parameter["min"] = {"Float": MINIMUM_DB}
    parameter["max"] = {"Float": MAXIMUM_DB}
    # The migrated pedals no longer define a step for these parameters.
    parameter.pop("step", None)


def _migrate_pedal(pedal_name: str, pedal: dict[str, Any]) -> None:
    parameters = pedal.get("parameters")
    if not isinstance(parameters, dict):
        return

    for parameter_name in LINEAR_DB_PARAMETERS.get(pedal_name, set()):
        parameter = parameters.get(parameter_name)
        if not isinstance(parameter, dict):
            continue
        value = _float_value(parameter)
        if value is not None:
            _set_float(parameter, linear_to_db(value))

    if pedal_name == "NoiseGate":
        threshold = parameters.pop("Threshold Db", None)
        if isinstance(threshold, dict):
            parameters["Threshold dB"] = threshold
        threshold = parameters.get("Threshold dB")
        if isinstance(threshold, dict):
            threshold["min"] = {"Float": MINIMUM_DB}
            threshold["max"] = {"Float": 0.0}

    if pedal_name == "Delay":
        decay = parameters.get("Decay")
        if isinstance(decay, dict):
            value = _float_value(decay)
            if value is not None:
                # Old decay was a unitless multiplier; the new value is the
                # positive dB loss per second. This is intentionally approximate.
                converted = 40.0 if value <= 0.0 else min(40.0, max(0.0, -20.0 * math.log10(value)))
                decay["value"] = {"Float": converted}
            decay["min"] = {"Float": 0.0}
            decay["max"] = {"Float": 40.0}


def _walk(value: Any) -> None:
    if isinstance(value, list):
        for item in value:
            _walk(item)
    elif isinstance(value, dict):
        for key, child in value.items():
            if key in LINEAR_DB_PARAMETERS or key in {"NoiseGate", "Delay"}:
                if isinstance(child, dict):
                    _migrate_pedal(key, child)
            else:
                _walk(child)


def migrate(data: Any) -> Any:
    result = copy.deepcopy(data)
    _walk(result)
    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="old pedalboards.json")
    parser.add_argument("output", type=Path, help="migrated pedalboards.json")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    with args.input.open("r", encoding="utf-8-sig") as input_file:
        data = json.load(input_file)
    migrated = migrate(data)
    with args.output.open("w", encoding="utf-8", newline="\n") as output_file:
        json.dump(migrated, output_file, indent=2)
        output_file.write("\n")


if __name__ == "__main__":
    main()
