"""Set OpenAPI `required` on object schemas from openapi/samples.json success bodies."""

from __future__ import annotations

import json
from collections import defaultdict
from pathlib import Path
from typing import Any

import yaml

ROOT = Path(__file__).resolve().parent
SPEC_PATH = ROOT / "nea-realtime.openapi.yaml"
SAMPLES_PATH = ROOT / "samples.json"

# operationId -> top-level key in samples.json
OPERATION_SAMPLE_KEY: dict[str, str] = {
    "psi": "psi",
    "pm25": "pm25",
    "air_temperature": "air-temperature",
    "relative_humidity": "relative-humidity",
    "wind_speed": "wind-speed",
    "wind_direction": "wind-direction",
    "rainfall": "rainfall",
    "two_hr_forecast": "two-hr-forecast",
    "twenty_four_hr_forecast": "twenty-four-hr-forecast",
    "four_day_outlook": "four-day-outlook",
    "uv": "uv",
}


def ref_name(reference: str) -> str:
    return reference.rsplit("/", 1)[-1]


def resolve_schema(
    fragment: dict[str, Any] | None, schemas: dict[str, Any]
) -> dict[str, Any] | None:
    if not fragment:
        return None
    if "$ref" in fragment:
        return schemas.get(ref_name(fragment["$ref"]))
    return fragment


def present_property_keys(sample: Any, property_names: Any) -> list[str]:
    if not isinstance(sample, dict):
        return []
    return [name for name in property_names if name in sample and sample[name] is not None]


def merge_required(
    accumulated: dict[str, set[str]], schema_name: str, keys: list[str]
) -> None:
    key_set = set(keys)
    if schema_name not in accumulated:
        accumulated[schema_name] = key_set
    else:
        accumulated[schema_name] &= key_set


def visit_schema(
    schema_name: str,
    sample: Any,
    schemas: dict[str, Any],
    accumulated: dict[str, set[str]],
) -> None:
    schema = schemas.get(schema_name)
    if not schema or not isinstance(schema, dict):
        return

    properties = schema.get("properties")
    if not isinstance(properties, dict) or not properties:
        return

    keys = present_property_keys(sample, properties)
    merge_required(accumulated, schema_name, keys)

    for wire_name in keys:
        child_sample = sample[wire_name]
        prop_fragment = properties[wire_name]
        prop_schema = resolve_schema(prop_fragment, schemas)
        if not isinstance(prop_schema, dict):
            if isinstance(prop_fragment, dict) and "$ref" in prop_fragment:
                visit_schema(ref_name(prop_fragment["$ref"]), child_sample, schemas, accumulated)
            continue

        if prop_schema.get("type") == "array":
            items = prop_schema.get("items")
            if isinstance(items, dict) and "$ref" in items:
                item_schema_name = ref_name(items["$ref"])
                if isinstance(child_sample, list):
                    for element in child_sample:
                        visit_schema(
                            item_schema_name, element, schemas, accumulated
                        )
            continue

        if isinstance(prop_fragment, dict) and "$ref" in prop_fragment:
            visit_schema(ref_name(prop_fragment["$ref"]), child_sample, schemas, accumulated)


def collect_required_from_samples(
    spec: dict[str, Any], samples: dict[str, Any]
) -> dict[str, list[str]]:
    schemas = spec.get("components", {}).get("schemas", {})
    if not isinstance(schemas, dict):
        return {}

    accumulated: dict[str, set[str]] = defaultdict(set)
    paths = spec.get("paths", {})
    if not isinstance(paths, dict):
        return {}

    for _path, methods in paths.items():
        if not isinstance(methods, dict):
            continue
        for _method, operation in methods.items():
            if not isinstance(operation, dict):
                continue
            operation_id = operation.get("operationId")
            if not isinstance(operation_id, str):
                continue
            sample_key = OPERATION_SAMPLE_KEY.get(operation_id)
            if not sample_key or sample_key not in samples:
                continue

            response = (
                operation.get("responses", {})
                .get("200", {})
                .get("content", {})
                .get("application/json", {})
                .get("schema", {})
            )
            if not isinstance(response, dict) or "$ref" not in response:
                continue

            visit_schema(
                ref_name(response["$ref"]),
                samples[sample_key],
                schemas,
                accumulated,
            )

    return {
        name: sorted(keys)
        for name, keys in accumulated.items()
        if keys
    }


def apply_required_to_spec(spec: dict[str, Any], samples: dict[str, Any]) -> int:
    """Write `required` arrays; return number of schemas updated."""
    required_by_schema = collect_required_from_samples(spec, samples)
    schemas = spec.setdefault("components", {}).setdefault("schemas", {})
    updated = 0

    for schema_name, required_keys in required_by_schema.items():
        schema = schemas.get(schema_name)
        if not isinstance(schema, dict):
            continue
        if not schema.get("properties"):
            continue
        schema["required"] = required_keys
        updated += 1

    return updated


def main() -> None:
    with SAMPLES_PATH.open(encoding="utf-8") as f:
        samples = json.load(f)
    if not isinstance(samples, dict):
        raise SystemExit(f"Expected object in {SAMPLES_PATH}")

    with SPEC_PATH.open(encoding="utf-8") as f:
        spec = yaml.safe_load(f)

    count = apply_required_to_spec(spec, samples)

    with SPEC_PATH.open("w", encoding="utf-8") as f:
        yaml.dump(
            spec,
            f,
            default_flow_style=False,
            sort_keys=False,
            allow_unicode=True,
            width=120,
        )

    print(f"Updated {SPEC_PATH}: required arrays on {count} schemas")


if __name__ == "__main__":
    main()
