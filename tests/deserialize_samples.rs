//! Deserialize captured NEA API success bodies from `tests/samples/*.json`.
//!
//! Failures are intentional signal — fix the generated types before merging a newly captured sample.
//! The scheduled upstream sampler opens draft pull requests with numbered regression fixtures.

use std::{
    fs,
    path::{Path, PathBuf},
};

use http::{HeaderMap, StatusCode};
use nea_rs::{
    AirTemperatureOperationResponse, FourDayOutlookOperationResponse, NeaHeatStressLevel,
    Pm25OperationResponse, PsiOperationResponse, RainfallOperationResponse,
    RelativeHumidityOperationResponse, TwentyFourHrForecastOperationResponse,
    TwoHrForecastOperationResponse, UvOperationResponse, WeatherSubApiOperationResponse,
    WindDirectionOperationResponse, WindSpeedOperationResponse,
    operations::{
        air_temperature::decode_air_temperature_response,
        four_day_outlook::decode_four_day_outlook_response, pm25::decode_pm25_response,
        psi::decode_psi_response, rainfall::decode_rainfall_response,
        relative_humidity::decode_relative_humidity_response,
        twenty_four_hr_forecast::decode_twenty_four_hr_forecast_response,
        two_hr_forecast::decode_two_hr_forecast_response, uv::decode_uv_response,
        weather_sub_api::decode_weather_sub_api_response,
        wind_direction::decode_wind_direction_response, wind_speed::decode_wind_speed_response,
    },
};

fn ok_response(body: &[u8]) -> satay_runtime::ResponseParts<Vec<u8>> {
    satay_runtime::ResponseParts {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: body.to_vec(),
    }
}

fn numbered_samples(prefix: &str) -> Vec<PathBuf> {
    let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/samples");
    let file_prefix = format!("{prefix}-");
    let mut samples = fs::read_dir(&samples_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", samples_dir.display()))
        .filter_map(|entry| {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "failed to read an entry in {}: {error}",
                    samples_dir.display()
                )
            });

            let file_type = entry.file_type().unwrap_or_else(|error| {
                panic!("failed to inspect {}: {error}", entry.path().display())
            });

            if !file_type.is_file() {
                return None;
            }

            let file_name = entry.file_name();
            let file_name = file_name.to_str().unwrap_or_else(|| {
                panic!("sample filename is not UTF-8: {}", entry.path().display())
            });

            let number = file_name
                .strip_prefix(&file_prefix)?
                .strip_suffix(".json")?
                .parse::<u64>()
                .unwrap_or_else(|error| {
                    panic!(
                        "sample filename must match {prefix}-<positive-number>.json: \
                         {file_name}: {error}"
                    )
                });
            assert!(
                number > 0,
                "sample number must be positive: {}",
                entry.path().display()
            );

            Some((number, entry.path()))
        })
        .collect::<Vec<_>>();

    samples.sort_by_key(|(number, _)| *number);

    for duplicate in samples.windows(2) {
        assert_ne!(
            duplicate[0].0,
            duplicate[1].0,
            "duplicate sample number {} for {} and {}",
            duplicate[0].0,
            duplicate[0].1.display(),
            duplicate[1].1.display()
        );
    }

    assert!(
        !samples.is_empty(),
        "no numbered samples found for {prefix} in {}",
        samples_dir.display()
    );

    samples.into_iter().map(|(_, path)| path).collect()
}

macro_rules! sample_deserializes {
    ($test_name:ident, $prefix:literal, $decode:path, $ok:pat) => {
        #[test]
        fn $test_name() {
            for path in numbered_samples($prefix) {
                let body = fs::read(&path)
                    .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

                let decoded = $decode(ok_response(&body))
                    .unwrap_or_else(|error| panic!("decode {} failed: {error}", path.display()));

                assert!(
                    matches!(decoded, $ok),
                    "expected Ok variant for {}",
                    path.display()
                );
            }
        }
    };
}

sample_deserializes!(psi, "psi", decode_psi_response, PsiOperationResponse::Ok(_));
sample_deserializes!(
    pm25,
    "pm25",
    decode_pm25_response,
    Pm25OperationResponse::Ok(_)
);
sample_deserializes!(
    air_temperature,
    "air-temperature",
    decode_air_temperature_response,
    AirTemperatureOperationResponse::Ok(_)
);
sample_deserializes!(
    relative_humidity,
    "relative-humidity",
    decode_relative_humidity_response,
    RelativeHumidityOperationResponse::Ok(_)
);
sample_deserializes!(
    wind_speed,
    "wind-speed",
    decode_wind_speed_response,
    WindSpeedOperationResponse::Ok(_)
);
sample_deserializes!(
    wind_direction,
    "wind-direction",
    decode_wind_direction_response,
    WindDirectionOperationResponse::Ok(_)
);
sample_deserializes!(
    rainfall,
    "rainfall",
    decode_rainfall_response,
    RainfallOperationResponse::Ok(_)
);
sample_deserializes!(
    two_hr_forecast,
    "two-hr-forecast",
    decode_two_hr_forecast_response,
    TwoHrForecastOperationResponse::Ok(_)
);
sample_deserializes!(
    twenty_four_hr_forecast,
    "twenty-four-hr-forecast",
    decode_twenty_four_hr_forecast_response,
    TwentyFourHrForecastOperationResponse::Ok(_)
);
sample_deserializes!(
    four_day_outlook,
    "four-day-outlook",
    decode_four_day_outlook_response,
    FourDayOutlookOperationResponse::Ok(_)
);
sample_deserializes!(uv, "uv", decode_uv_response, UvOperationResponse::Ok(_));
sample_deserializes!(
    weather_lightning,
    "weather-lightning",
    decode_weather_sub_api_response,
    WeatherSubApiOperationResponse::Ok(_)
);
sample_deserializes!(
    weather_wbgt,
    "weather-wbgt",
    decode_weather_sub_api_response,
    WeatherSubApiOperationResponse::Ok(_)
);

#[test]
fn weather_wbgt_not_available_sentinels_deserialize() {
    let body = br#"{
        "code": 0,
        "data": {
            "records": [{
                "item": {
                    "readings": [
                        {"wbgt": "NA", "heatStress": "NA"},
                        {"wbgt": "28.7", "heatStress": "Low"}
                    ]
                }
            }]
        }
    }"#;

    let decoded = decode_weather_sub_api_response(ok_response(body))
        .expect("WBGT response with NA sentinels should decode");
    let WeatherSubApiOperationResponse::Ok(response) = decoded else {
        panic!("expected Ok variant");
    };
    let readings = response
        .data
        .and_then(|data| data.records)
        .and_then(|records| records.into_iter().next())
        .and_then(|record| record.item)
        .and_then(|item| item.readings)
        .expect("expected WBGT readings");

    assert_eq!(readings[0].wbgt, None);
    assert_eq!(
        readings[0].heat_stress,
        Some(NeaHeatStressLevel::NotAvailable)
    );
    assert_eq!(readings[1].wbgt, Some(28.7));
    assert_eq!(readings[1].heat_stress, Some(NeaHeatStressLevel::Low));
}
