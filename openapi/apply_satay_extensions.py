#!/usr/bin/env python3
"""Apply x-satay shared schemas and $ref wiring to nea-realtime.openapi.yaml.

Ranges for numeric bounds are derived from openapi/samples.json where present,
with conservative headroom for fields not covered by samples.
"""

from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

import yaml

# Bare https:// in OpenAPI descriptions become rustdoc `///` lines; wrap for clickable links.
BARE_URL_RE = re.compile(r"(?<!<)(https://[^\s<>\')\]]+)")

ROOT = Path(__file__).resolve().parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from apply_required_from_samples import apply_required_to_spec
SPEC_PATH = ROOT / "nea-realtime.openapi.yaml"
SAMPLES_PATH = ROOT / "samples.json"

# Calendar dates (YYYY-MM-DD) in response bodies and query parameters.
DATE_PROPERTIES = frozenset({"date"})

# Property names treated as ISO-8601 timestamps (wire format stays string).
DATETIME_PROPERTIES = frozenset(
    {
        "timestamp",
        "updatedTimestamp",
        "update_timestamp",
        "datetime",
        "hour",
        "start",
        "end",
    }
)

LATITUDE_PROPERTIES = frozenset({"latitude"})
LONGITUDE_PROPERTIES = frozenset({"longitude"})

REGIONAL_KEYS = frozenset({"east", "west", "north", "south", "central"})

# NEA real-time weather station wire IDs (e.g. S06, S109, S900) from samples.json.
STATION_ID_PATTERN = r"^S[0-9]{2,3}$"
STATION_ID_PROPERTIES = frozenset({"id", "deviceId", "stationId"})
STATION_ID_SCHEMAS = frozenset({"NeaWeatherStation", "NeaStationReading"})

# Weather station list entries (air-temp, humidity, wind, rainfall) use wire key `location`.
WEATHER_STATION_SCHEMA = "NeaWeatherStation"

# Unit-of-measure strings from station readings and MSS outlook ranges (samples).
MEASUREMENT_UNIT_ENUM = [
    "deg C",
    "Degrees Celsius",
    "Percentage",
    "percentage",
    "knots",
    "degrees",
    "mm",
]
MEASUREMENT_UNIT_VARIANTS = {
    "deg C": "DegC",
    "Degrees Celsius": "DegreesCelsius",
    "Percentage": "Percentage",
    "percentage": "PercentLower",
    "knots": "Knots",
    "degrees": "Degrees",
    "mm": "Millimeters",
}

STATION_DATA_SCHEMAS_WITH_READING_UNIT = frozenset(
    {
        "AirTemperatureData",
        "RelativeHumidityData",
        "WindSpeedData",
        "WindDirectionData",
        "RainfallData",
    }
)

REMOVED_UNIT_SCHEMAS = frozenset(
    {
        "NeaAirTemperatureReadingUnit",
        "NeaTemperatureUnit",
        "NeaHumidityUnit",
    }
)

# 16-point compass rose used in 4-day / 24-hour outlook wind.direction.
WIND_DIRECTION_16_ENUM = [
    "N",
    "NNE",
    "NE",
    "ENE",
    "E",
    "ESE",
    "SE",
    "SSE",
    "S",
    "SSW",
    "SW",
    "WSW",
    "W",
    "WNW",
    "NW",
    "NNW",
]
WIND_DIRECTION_16_VARIANTS = {value: value for value in WIND_DIRECTION_16_ENUM}

# NEA MSS forecast codes seen in samples and historical datasets.
FORECAST_CODE_ENUM = [
    "FA",
    "FN",
    "FW",
    "CL",
    "PC",
    "PN",
    "HZ",
    "SH",
    "WI",
    "MS",
    "FG",
    "LR",
    "MR",
    "HR",
    "PS",
    "LS",
    "HS",
    "TS",
    "HT",
    "HG",
    "TL",
]

FORECAST_CODE_VARIANTS = {
    "FA": "FairDay",
    "FN": "FairNight",
    "FW": "FairAndWarm",
    "CL": "Cloudy",
    "PC": "PartlyCloudyDay",
    "PN": "PartlyCloudyNight",
    "HZ": "Hazy",
    "SH": "SlightlyHazy",
    "WI": "Windy",
    "MS": "Mist",
    "FG": "Fog",
    "LR": "LightRain",
    "MR": "ModerateRain",
    "HR": "HeavyRain",
    "PS": "PassingShowers",
    "LS": "LightShowers",
    "HS": "HeavyShowers",
    "TS": "ThunderyShowers",
    "HT": "HeavyThunderyShowers",
    "HG": "HeavyThunderyShowersWithGustyWinds",
    "TL": "ThunderyShowersAlt",
}

# NEA MSS human-readable forecast strings (2h / 24h / 4-day outlook).
FORECAST_TEXT_ENUM = [
    "Fair",
    "Fair (Day)",
    "Fair (Night)",
    "Fair and Warm",
    "Partly Cloudy",
    "Partly Cloudy (Day)",
    "Partly Cloudy (Night)",
    "Cloudy",
    "Hazy",
    "Slightly Hazy",
    "Windy",
    "Mist",
    "Fog",
    "Light Rain",
    "Moderate Rain",
    "Heavy Rain",
    "Passing Showers",
    "Light Showers",
    "Showers",
    "Heavy Showers",
    "Thundery Showers",
    "Heavy Thundery Showers",
    "Heavy Thundery Showers with Gusty Winds",
]

FORECAST_TEXT_VARIANTS = {
    "Fair": "Fair",
    "Fair (Day)": "FairDay",
    "Fair (Night)": "FairNight",
    "Fair and Warm": "FairAndWarm",
    "Partly Cloudy": "PartlyCloudy",
    "Partly Cloudy (Day)": "PartlyCloudyDay",
    "Partly Cloudy (Night)": "PartlyCloudyNight",
    "Cloudy": "Cloudy",
    "Hazy": "Hazy",
    "Slightly Hazy": "SlightlyHazy",
    "Windy": "Windy",
    "Mist": "Mist",
    "Fog": "Fog",
    "Light Rain": "LightRain",
    "Moderate Rain": "ModerateRain",
    "Heavy Rain": "HeavyRain",
    "Passing Showers": "PassingShowers",
    "Light Showers": "LightShowers",
    "Showers": "Showers",
    "Heavy Showers": "HeavyShowers",
    "Thundery Showers": "ThunderyShowers",
    "Heavy Thundery Showers": "HeavyThunderyShowers",
    "Heavy Thundery Showers with Gusty Winds": "HeavyThunderyShowersWithGustyWinds",
}

DAY_OF_WEEK_ENUM = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
]

DAY_OF_WEEK_VARIANTS = {day: day for day in DAY_OF_WEEK_ENUM}

INVALID_PARAMS_ERROR_MSG_ENUM = [
    "Invalid date format. Date format must be YYYY-MM-DD (2024-06-01) or YYYY-MM-DDTHH:mm:ss (2024-06-01T08:30:00).",
    "Invalid pagination token.",
]

INVALID_PARAMS_ERROR_MSG_VARIANTS = {
    INVALID_PARAMS_ERROR_MSG_ENUM[0]: "InvalidDateFormat",
    INVALID_PARAMS_ERROR_MSG_ENUM[1]: "InvalidPaginationToken",
}

WEATHER_SUB_API_ENUM = ["lightning", "wbgt"]

WEATHER_SUB_API_VARIANTS = {
    "lightning": "Lightning",
    "wbgt": "WetBulbGlobeTemperature",
}

SHARED_SCHEMAS = {
    "NeaSuccessCode": {
        "type": "integer",
        "description": "Response status code (0 for success)",
        "minimum": 0,
        "maximum": 0,
        "example": 0,
    },
    "NeaOffsetDateTime": {
        "type": "string",
        "description": "ISO 8601 date or date-time in Singapore Time (SGT)",
        "x-satay": {"parse-as": "offset-datetime"},
    },
    "NeaDate": {
        "type": "string",
        "description": "SGT calendar date (YYYY-MM-DD)",
        "example": "2024-07-16",
        "x-satay": {"parse-as": "date"},
    },
    "NeaLatitude": {
        "type": "number",
        "description": "WGS84 latitude for Singapore",
        "minimum": 1.0,
        "maximum": 1.5,
    },
    "NeaLongitude": {
        "type": "number",
        "description": "WGS84 longitude for Singapore",
        "minimum": 103.5,
        "maximum": 104.2,
    },
    "NeaStringLatitude": {
        "type": "string",
        "description": "WGS84 latitude as a decimal string",
        "x-satay": {"parse-as": "f64"},
    },
    "NeaStringLongitude": {
        "type": "string",
        "description": "WGS84 longitude as a decimal string",
        "x-satay": {"parse-as": "f64"},
    },
    "NeaStationId": {
        "type": "string",
        "description": "NEA weather station identifier (S + 2–3 digits)",
        "pattern": STATION_ID_PATTERN,
        "minLength": 3,
        "maxLength": 4,
        "example": "S109",
    },
    "NeaStationLocation": {
        "type": "object",
        "description": "WGS84 coordinates for a weather station",
        "properties": {
            "latitude": {"$ref": "#/components/schemas/NeaLatitude"},
            "longitude": {"$ref": "#/components/schemas/NeaLongitude"},
        },
        "required": ["latitude", "longitude"],
    },
    "NeaRegionalReading": {
        "type": "integer",
        "description": "Regional air-quality or index reading",
        "minimum": 0,
        "maximum": 500,
    },
    "NeaPm25HourlyReading": {
        "type": "integer",
        "description": "Regional PM2.5 one-hour reading (µg/m³)",
        "minimum": 0,
        "maximum": 500,
    },
    "NeaWindDirectionDegrees": {
        "type": "integer",
        "description": "Wind direction in degrees",
        "minimum": 0,
        "maximum": 360,
    },
    "NeaUvIndex": {
        "type": "integer",
        "description": "UV index for the hour",
        "minimum": 0,
        "maximum": 16,
    },
    "NeaHumidityPercent": {
        "type": "number",
        "description": "Relative humidity percentage",
        "minimum": 0,
        "maximum": 100,
    },
    "NeaTemperatureCelsius": {
        "type": "number",
        "description": "Air temperature in degrees Celsius",
        "minimum": 15,
        "maximum": 45,
    },
    "NeaMeasurementUnit": {
        "type": "string",
        "description": "Unit of measure for NEA station readings and outlook ranges",
        "enum": MEASUREMENT_UNIT_ENUM,
        "example": "deg C",
        "x-satay": {"enum-variants": MEASUREMENT_UNIT_VARIANTS},
    },
    "NeaWindDirection16": {
        "type": "string",
        "description": "16-point compass wind direction for outlook forecasts",
        "enum": WIND_DIRECTION_16_ENUM,
        "example": "SSE",
        "x-satay": {"enum-variants": WIND_DIRECTION_16_VARIANTS},
    },
    "NeaWindSpeedKmh": {
        "type": "number",
        "description": "Wind speed in km/h",
        "minimum": 0,
        "maximum": 120,
    },
    "NeaForecastCode": {
        "type": "string",
        "description": "NEA MSS short weather forecast code",
        "enum": FORECAST_CODE_ENUM,
        "x-satay": {"enum-variants": FORECAST_CODE_VARIANTS},
    },
    "NeaLightningType": {
        "type": "string",
        "description": "Lightning event type (C = cloud-to-cloud, G = cloud-to-ground)",
        "enum": ["C", "G"],
        "x-satay": {
            "enum-variants": {
                "C": "CloudToCloud",
                "G": "CloudToGround",
            }
        },
    },
    "NeaForecastText": {
        "type": "string",
        "description": "NEA MSS human-readable weather forecast text",
        "enum": FORECAST_TEXT_ENUM,
        "x-satay": {"enum-variants": FORECAST_TEXT_VARIANTS},
    },
    "NeaDayOfWeek": {
        "type": "string",
        "description": "Day of week for multi-day outlook forecasts",
        "enum": DAY_OF_WEEK_ENUM,
        "x-satay": {"enum-variants": DAY_OF_WEEK_VARIANTS},
    },
    "NeaInvalidParamsErrorMsg": {
        "type": "string",
        "description": "Bad-request error message for invalid date or pagination token",
        "enum": INVALID_PARAMS_ERROR_MSG_ENUM,
        "x-satay": {"enum-variants": INVALID_PARAMS_ERROR_MSG_VARIANTS},
    },
    "NeaWeatherSubApi": {
        "type": "string",
        "description": "Weather sub-API selector (lightning or wbgt)",
        "enum": WEATHER_SUB_API_ENUM,
        "x-satay": {"enum-variants": WEATHER_SUB_API_VARIANTS},
    },
}

# Schemas whose integer region properties share NeaRegionalReading.
PSI_REGIONAL_SCHEMAS = {
    "PsiCoEightHourMaxRegional",
    "PsiCoSubIndexRegional",
    "PsiNo2OneHourMaxRegional",
    "PsiO3EightHourMaxRegional",
    "PsiO3SubIndexRegional",
    "PsiPm10SubIndexRegional",
    "PsiPm10TwentyFourHourRegional",
    "PsiPm25SubIndexRegional",
    "PsiPm25TwentyFourHourRegional",
    "PsiThreeHourRegional",
    "PsiTwentyFourHourRegional",
    "PsiSo2SubIndexRegional",
    "PsiSo2TwentyFourHourRegional",
}

PM25_REGIONAL_SCHEMAS = {"Pm25OneHourRegional"}

# Duplicate component schemas collapsed to one canonical name (first alias wins).
SCHEMA_DEDUPE: dict[str, tuple[str, ...]] = {
    "NeaWeatherStation": (
        "AirTemperatureResponseDataStationsItems",
        "RelativeHumidityResponseDataStationsItems",
        "WindSpeedResponseDataStationsItems",
        "WindDirectionResponseDataStationsItems",
        "RainfallResponseDataStationsItems",
    ),
    "NeaReadingSnapshot": (
        "AirTemperatureResponseDataReadingsItems",
        "RelativeHumidityResponseDataReadingsItems",
        "WindSpeedResponseDataReadingsItems",
        "WindDirectionResponseDataReadingsItems",
        "RainfallResponseDataReadingsItems",
    ),
    "NeaStationReading": (
        "AirTemperatureResponseDataReadingsItemsDataItems",
        "RelativeHumidityResponseDataReadingsItemsDataItems",
        "WindSpeedResponseDataReadingsItemsDataItems",
        "WindDirectionResponseDataReadingsItemsDataItems",
        "RainfallResponseDataReadingsItemsDataItems",
    ),
    "NeaHumidityRange": (
        "FourDayOutlookResponseDataRecordsItemsForecastsItemsRelativeHumidity",
        "TwentyFourHrForecastResponseDataRecordsItemsGeneralRelativeHumidity",
    ),
    "NeaTemperatureRange": (
        "FourDayOutlookResponseDataRecordsItemsForecastsItemsTemperature",
        "TwentyFourHrForecastResponseDataRecordsItemsGeneralTemperature",
    ),
    "NeaWindSpeedRange": (
        "FourDayOutlookResponseDataRecordsItemsForecastsItemsWindSpeed",
        "TwentyFourHrForecastResponseDataRecordsItemsGeneralWindSpeed",
    ),
    "NeaOutlookWind": (
        "FourDayOutlookResponseDataRecordsItemsForecastsItemsWind",
        "TwentyFourHrForecastResponseDataRecordsItemsGeneralWind",
    ),
    "NeaMssForecast": (
        "TwentyFourHrForecastResponseDataRecordsItemsGeneralForecast",
    ),
    "NeaGeoPoint": (
        "PsiResponseDataRegionMetadataItemsLabelLocation",
        "TwoHrForecastResponseDataAreaMetadataItemsLabelLocation",
        "WeatherSubApiResponseDataRecordsItemsItemReadingsItemsLocation",
    ),
}

# Unique schema renames (old OpenAPI generator name -> descriptive Rust type name).
SCHEMA_RENAMES: dict[str, str] = {
    # Real-time weather station endpoints
    "AirTemperatureResponseData": "AirTemperatureData",
    "RelativeHumidityResponseData": "RelativeHumidityData",
    "WindSpeedResponseData": "WindSpeedData",
    "WindDirectionResponseData": "WindDirectionData",
    "RainfallResponseData": "RainfallData",
    # Four-day outlook
    "FourDayOutlookResponseData": "FourDayOutlookData",
    "FourDayOutlookResponseDataRecordsItems": "FourDayOutlookDay",
    "FourDayOutlookResponseDataRecordsItemsForecastsItems": "FourDayOutlookPeriod",
    "FourDayOutlookResponseDataRecordsItemsForecastsItemsForecast": "NeaOutlookForecastDetail",
    # Twenty-four-hour forecast
    "TwentyFourHrForecastResponseData": "TwentyFourHrForecastData",
    "TwentyFourHrForecastResponseDataRecordsItems": "TwentyFourHrForecastDay",
    "TwentyFourHrForecastResponseDataRecordsItemsGeneral": "TwentyFourHrForecastGeneral",
    "TwentyFourHrForecastResponseDataRecordsItemsPeriodsItems": "TwentyFourHrForecastPeriod",
    "TwentyFourHrForecastResponseDataRecordsItemsPeriodsItemsRegions": "TwentyFourHrRegionalForecast",
    # Two-hour forecast
    "TwoHrForecastResponseData": "TwoHrForecastData",
    "TwoHrForecastResponseDataAreaMetadataItems": "NeaForecastArea",
    "TwoHrForecastResponseDataItemsItems": "TwoHrForecastSnapshot",
    "TwoHrForecastResponseDataItemsItemsForecastsItems": "TwoHrAreaForecast",
    "TwoHrForecastResponseDataItemsItemsValidPeriod": "NeaValidPeriod",
    # PSI / PM2.5
    "PsiResponseData": "PsiData",
    "PsiResponseDataItemsItems": "PsiSnapshot",
    "PsiResponseDataItemsItemsReadings": "PsiReadings",
    "PsiResponseDataRegionMetadataItems": "NeaRegionMetadata",
    "PsiResponseDataItemsItemsReadingsCoEightHourMax": "PsiCoEightHourMaxRegional",
    "PsiResponseDataItemsItemsReadingsCoSubIndex": "PsiCoSubIndexRegional",
    "PsiResponseDataItemsItemsReadingsNo2OneHourMax": "PsiNo2OneHourMaxRegional",
    "PsiResponseDataItemsItemsReadingsO3EightHourMax": "PsiO3EightHourMaxRegional",
    "PsiResponseDataItemsItemsReadingsO3SubIndex": "PsiO3SubIndexRegional",
    "PsiResponseDataItemsItemsReadingsPm10SubIndex": "PsiPm10SubIndexRegional",
    "PsiResponseDataItemsItemsReadingsPm10TwentyFourHourly": "PsiPm10TwentyFourHourRegional",
    "PsiResponseDataItemsItemsReadingsPm25SubIndex": "PsiPm25SubIndexRegional",
    "PsiResponseDataItemsItemsReadingsPm25TwentyFourHourly": "PsiPm25TwentyFourHourRegional",
    "PsiResponseDataItemsItemsReadingsPsiThreeHourly": "PsiThreeHourRegional",
    "PsiResponseDataItemsItemsReadingsPsiTwentyFourHourly": "PsiTwentyFourHourRegional",
    "PsiResponseDataItemsItemsReadingsSo2SubIndex": "PsiSo2SubIndexRegional",
    "PsiResponseDataItemsItemsReadingsSo2TwentyFourHourly": "PsiSo2TwentyFourHourRegional",
    "Pm25ResponseData": "Pm25Data",
    "Pm25ResponseDataItemsItems": "Pm25Snapshot",
    "Pm25ResponseDataItemsItemsReadings": "Pm25Readings",
    "Pm25ResponseDataItemsItemsReadingsPm25OneHourly": "Pm25OneHourRegional",
    # UV
    "UvResponseData": "UvData",
    "UvResponseDataRecordsItems": "UvDayRecord",
    "UvResponseDataRecordsItemsIndexItems": "UvHourlyIndex",
    # Weather sub-API (lightning / WBGT)
    "WeatherSubApiResponseData": "WeatherSubApiData",
    "WeatherSubApiResponseDataRecordsItems": "WeatherSubApiDayRecord",
    "WeatherSubApiResponseDataRecordsItemsItem": "WeatherSubApiObservation",
    "WeatherSubApiResponseDataRecordsItemsItemReadingsItems": "WeatherSubApiLightningReading",
    # Errors
    "InvalidParamsError2": "WeatherSubApiInvalidParamsError",
}

SUCCESS_RESPONSE_SCHEMAS = {
    "PsiResponse",
    "Pm25Response",
    "AirTemperatureResponse",
    "RelativeHumidityResponse",
    "WindSpeedResponse",
    "WindDirectionResponse",
    "RainfallResponse",
    "TwoHrForecastResponse",
    "TwentyFourHrForecastResponse",
    "FourDayOutlookResponse",
    "UvResponse",
    "WeatherSubApiResponse",
}


def load_station_ids_from_samples() -> set[str]:
    with SAMPLES_PATH.open(encoding="utf-8") as f:
        samples = json.load(f)

    ids: set[str] = set()

    def walk(obj: object) -> None:
        if isinstance(obj, dict):
            for key, value in obj.items():
                if key in STATION_ID_PROPERTIES and isinstance(value, str):
                    ids.add(value)
                walk(value)
        elif isinstance(obj, list):
            for value in obj:
                walk(value)

    walk(samples)
    return ids


def validate_station_id_pattern(ids: set[str]) -> None:
    pattern = re.compile(STATION_ID_PATTERN)
    invalid = sorted(station_id for station_id in ids if pattern.fullmatch(station_id) is None)
    if invalid:
        raise ValueError(
            "samples.json station IDs do not match "
            f"{STATION_ID_PATTERN!r}: {', '.join(invalid)}"
        )


def load_samples_ranges() -> dict[str, dict[str, float]]:
    """Return min/max for numeric leaf paths in samples.json."""
    with SAMPLES_PATH.open(encoding="utf-8") as f:
        samples = json.load(f)

    stats: dict[str, list[float]] = defaultdict(list)

    def walk(obj, path: str = "") -> None:
        if isinstance(obj, dict):
            for key, value in obj.items():
                walk(value, f"{path}.{key}" if path else key)
        elif isinstance(obj, list):
            for value in obj:
                walk(value, f"{path}[]")
        elif isinstance(obj, bool):
            return
        elif isinstance(obj, int):
            stats[path].append(float(obj))
        elif isinstance(obj, float):
            stats[path].append(obj)

    walk(samples)
    return {
        path: {"min": min(values), "max": max(values)}
        for path, values in stats.items()
        if values
    }


def ref(name: str) -> dict:
    return {"$ref": f"#/components/schemas/{name}"}


def is_forecast_text_enum(prop_schema: dict) -> bool:
    values = prop_schema.get("enum")
    return isinstance(values, list) and values == FORECAST_TEXT_ENUM


def patch_weather_api_query_parameter(spec: dict) -> None:
    parameters = spec.get("components", {}).get("parameters", {})
    weather_api = parameters.get("WeatherApiQuery")
    if not isinstance(weather_api, dict):
        return
    weather_api.pop("x-satay", None)
    schema = weather_api.setdefault("schema", {})
    if isinstance(schema, dict):
        schema.clear()
        schema.update(ref("NeaWeatherSubApi"))


ORPHAN_STATION_LOCATION_SCHEMAS = frozenset(
    {
        "AirTemperatureResponseDataStationsItemsLabelLocation",
        "RainfallResponseDataStationsItemsLabelLocation",
        "RelativeHumidityResponseDataStationsItemsLabelLocation",
        "WindSpeedResponseDataStationsItemsLocation",
    }
)


def schema_ref_map() -> dict[str, str]:
    mapping: dict[str, str] = dict(SCHEMA_RENAMES)
    for canonical, aliases in SCHEMA_DEDUPE.items():
        for alias in aliases:
            mapping[alias] = canonical
    return mapping


def rewrite_schema_refs(node: object, ref_map: dict[str, str]) -> None:
    if isinstance(node, dict):
        reference = node.get("$ref")
        if isinstance(reference, str) and reference.startswith("#/components/schemas/"):
            old_name = reference.rsplit("/", 1)[-1]
            if old_name in ref_map:
                node["$ref"] = f"#/components/schemas/{ref_map[old_name]}"
        for value in node.values():
            rewrite_schema_refs(value, ref_map)
    elif isinstance(node, list):
        for item in node:
            rewrite_schema_refs(item, ref_map)


def apply_schema_names(spec: dict) -> tuple[int, int]:
    """Rename verbose generated schema names and deduplicate identical shapes."""
    schemas = spec.get("components", {}).get("schemas", {})
    if not isinstance(schemas, dict):
        return 0, 0

    renamed = 0
    deduped = 0

    for canonical, aliases in SCHEMA_DEDUPE.items():
        source = next((alias for alias in aliases if alias in schemas), None)
        if source is None:
            continue
        if canonical not in schemas:
            if source != canonical:
                schemas[canonical] = schemas.pop(source)
            renamed += 1
        for alias in aliases:
            if alias in schemas and alias != canonical:
                schemas.pop(alias)
                deduped += 1

    for old_name, new_name in SCHEMA_RENAMES.items():
        if old_name not in schemas or old_name == new_name:
            continue
        if new_name in schemas:
            schemas.pop(old_name)
            deduped += 1
            continue
        schemas[new_name] = schemas.pop(old_name)
        renamed += 1

    rewrite_schema_refs(spec, schema_ref_map())
    return renamed, deduped


def remove_orphan_station_location_schemas(schemas: dict) -> None:
    for name in ORPHAN_STATION_LOCATION_SCHEMAS:
        schemas.pop(name, None)


def remove_replaced_unit_schemas(schemas: dict) -> None:
    for name in REMOVED_UNIT_SCHEMAS:
        schemas.pop(name, None)


def patch_weather_station_items_schema(name: str, schema: dict) -> None:
    """Wire `location` + shared NeaStationLocation (samples use `location`, not `labelLocation`)."""
    if name != WEATHER_STATION_SCHEMA:
        return
    props = schema.setdefault("properties", {})
    props.pop("labelLocation", None)
    props["location"] = ref("NeaStationLocation")
    required = schema.setdefault("required", [])
    if "location" not in required:
        required.append("location")
        required.sort()


def patch_regional_object(schema: dict, ref_name: str) -> None:
    props = schema.get("properties")
    if not props:
        return
    for key in REGIONAL_KEYS:
        if key in props and props[key].get("type") == "integer":
            props[key] = ref(ref_name)


def linkify_description_urls(text: str) -> str:
    return BARE_URL_RE.sub(r"<\1>", text)


def linkify_spec_descriptions(node: object) -> None:
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "description" and isinstance(value, str):
                node[key] = linkify_description_urls(value)
            else:
                linkify_spec_descriptions(value)
    elif isinstance(node, list):
        for item in node:
            linkify_spec_descriptions(item)


def transform_schema(name: str, schema: dict) -> None:
    if not isinstance(schema, dict):
        return

    patch_weather_station_items_schema(name, schema)

    if name in SUCCESS_RESPONSE_SCHEMAS:
        props = schema.get("properties", {})
        if "code" in props:
            props["code"] = ref("NeaSuccessCode")

    if name in PSI_REGIONAL_SCHEMAS:
        patch_regional_object(schema, "NeaRegionalReading")

    if name in PM25_REGIONAL_SCHEMAS:
        patch_regional_object(schema, "NeaPm25HourlyReading")

    if name in STATION_DATA_SCHEMAS_WITH_READING_UNIT:
        props = schema.get("properties", {})
        if "readingUnit" in props:
            props["readingUnit"] = ref("NeaMeasurementUnit")

    props = schema.get("properties")
    if props:
        for prop_name, prop_schema in list(props.items()):
            if not isinstance(prop_schema, dict):
                continue

            if prop_name in DATE_PROPERTIES and prop_schema.get("type") == "string":
                props[prop_name] = ref("NeaDate")
                continue

            if prop_name in DATETIME_PROPERTIES and prop_schema.get("type") == "string":
                props[prop_name] = ref("NeaOffsetDateTime")
                continue

            if prop_name in LATITUDE_PROPERTIES:
                if prop_schema.get("type") == "number":
                    props[prop_name] = ref("NeaLatitude")
                elif prop_schema.get("type") == "string":
                    props[prop_name] = ref("NeaStringLatitude")
                continue

            if prop_name in LONGITUDE_PROPERTIES:
                if prop_schema.get("type") == "number":
                    props[prop_name] = ref("NeaLongitude")
                elif prop_schema.get("type") == "string":
                    props[prop_name] = ref("NeaStringLongitude")
                continue

            if (
                prop_name in STATION_ID_PROPERTIES
                and prop_schema.get("type") == "string"
                and name in STATION_ID_SCHEMAS
            ):
                props[prop_name] = ref("NeaStationId")
                continue

            # Forecast code fields (not API error codes).
            if (
                prop_name == "code"
                and prop_schema.get("type") == "string"
                and "Forecast" in name
            ):
                props[prop_name] = ref("NeaForecastCode")
                continue

            if prop_name == "day" and prop_schema.get("enum") == DAY_OF_WEEK_ENUM:
                props[prop_name] = ref("NeaDayOfWeek")
                continue

            if prop_name in {"text", "forecast"} and is_forecast_text_enum(prop_schema):
                props[prop_name] = ref("NeaForecastText")
                continue

            if name == "InvalidParamsError" and prop_name == "errorMsg":
                if prop_schema.get("enum") == INVALID_PARAMS_ERROR_MSG_ENUM:
                    props[prop_name] = ref("NeaInvalidParamsErrorMsg")
                    continue

            if prop_name == "type" and name.endswith("ReadingsItems"):
                if prop_schema.get("type") == "string" and prop_schema.get("example") in {
                    "C",
                    "G",
                }:
                    props[prop_name] = ref("NeaLightningType")

            if prop_name == "value":
                parent_hints = name.lower()
                if "winddirection" in parent_hints:
                    if prop_schema.get("type") in {"integer", "number"}:
                        props[prop_name] = ref("NeaWindDirectionDegrees")
                elif "uv" in parent_hints and "index" in parent_hints:
                    if prop_schema.get("type") in {"integer", "number"}:
                        props[prop_name] = ref("NeaUvIndex")

            if prop_name in {"low", "high"}:
                if "relativehumidity" in name.lower() or "humidity" in name.lower():
                    if prop_schema.get("type") == "number":
                        props[prop_name] = ref("NeaHumidityPercent")
                elif "temperature" in name.lower():
                    if prop_schema.get("type") == "number":
                        props[prop_name] = ref("NeaTemperatureCelsius")
                elif "windspeed" in name.lower():
                    if prop_schema.get("type") in {"integer", "number"}:
                        props[prop_name] = ref("NeaWindSpeedKmh")

            if prop_name == "unit" and (
                "Temperature" in name
                or "Humidity" in name
                or "RelativeHumidity" in name
            ):
                props[prop_name] = ref("NeaMeasurementUnit")
                continue

            if prop_name == "direction" and "Wind" in name:
                props[prop_name] = ref("NeaWindDirection16")
                continue

    # Recurse into nested schemas referenced inline (rare) and array items.
    for key, value in schema.items():
        if key == "properties":
            continue
        if isinstance(value, dict):
            transform_schema(f"{name}.{key}", value)


def main() -> None:
    validate_station_id_pattern(load_station_ids_from_samples())
    sample_ranges = load_samples_ranges()
    # Tighten wind-direction bounds from samples when available.
    # Wind direction stays 0–360; samples only validate deserialization smoke tests.

    uv_paths = [p for p in sample_ranges if p.endswith("index[].value")]
    if uv_paths:
        vals = [sample_ranges[p] for p in uv_paths]
        lo = int(min(v["min"] for v in vals))
        hi = int(max(v["max"] for v in vals))
        SHARED_SCHEMAS["NeaUvIndex"]["minimum"] = max(0, lo)
        SHARED_SCHEMAS["NeaUvIndex"]["maximum"] = max(16, hi + 2)

    with SPEC_PATH.open(encoding="utf-8") as f:
        spec = yaml.safe_load(f)

    schemas = spec.setdefault("components", {}).setdefault("schemas", {})
    for shared_name, shared_schema in SHARED_SCHEMAS.items():
        schemas[shared_name] = shared_schema

    renamed, deduped = apply_schema_names(spec)

    for schema_name, schema in list(schemas.items()):
        if schema_name in SHARED_SCHEMAS:
            continue
        transform_schema(schema_name, schema)

    patch_weather_api_query_parameter(spec)
    remove_orphan_station_location_schemas(schemas)
    remove_replaced_unit_schemas(schemas)

    with SAMPLES_PATH.open(encoding="utf-8") as f:
        samples = json.load(f)
    required_count = apply_required_to_spec(spec, samples)
    linkify_spec_descriptions(spec)

    with SPEC_PATH.open("w", encoding="utf-8") as f:
        yaml.dump(
            spec,
            f,
            default_flow_style=False,
            sort_keys=False,
            allow_unicode=True,
            width=120,
        )

    print(f"Updated {SPEC_PATH}")
    print(f"Shared schemas: {', '.join(SHARED_SCHEMAS)}")
    print(f"Schema renames: {renamed}, deduplicated: {deduped}")
    print(f"Required arrays from samples: {required_count} schemas")


if __name__ == "__main__":
    main()
