//! Deserialize captured NEA API success bodies from `tests/samples/*.json`.
//!
//! Failures are intentional signal — fix the generated types or refresh samples later.

use http::{HeaderMap, StatusCode};
use nea_rs::{
    decode_air_temperature_response, decode_four_day_outlook_response, decode_pm25_response,
    decode_psi_response, decode_rainfall_response, decode_relative_humidity_response,
    decode_twenty_four_hr_forecast_response, decode_two_hr_forecast_response, decode_uv_response,
    decode_wind_direction_response, decode_wind_speed_response, AirTemperatureOperationResponse,
    FourDayOutlookOperationResponse, Pm25OperationResponse, PsiOperationResponse,
    RainfallOperationResponse, RelativeHumidityOperationResponse,
    TwentyFourHrForecastOperationResponse, TwoHrForecastOperationResponse, UvOperationResponse,
    WindDirectionOperationResponse, WindSpeedOperationResponse,
};

fn ok_response(body: &[u8]) -> satay_runtime::ResponseParts<Vec<u8>> {
    satay_runtime::ResponseParts {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: body.to_vec(),
    }
}

macro_rules! sample_deserializes {
    ($test_name:ident, $file:literal, $decode:path, $ok:pat) => {
        #[test]
        fn $test_name() {
            let body = include_str!(concat!("samples/", $file));
            let decoded = $decode(ok_response(body.as_bytes()))
                .unwrap_or_else(|e| panic!("decode {} failed: {e}", $file));
            assert!(matches!(decoded, $ok), "expected Ok variant for {}", $file);
        }
    };
}

sample_deserializes!(psi, "psi.json", decode_psi_response, PsiOperationResponse::Ok(_));
sample_deserializes!(pm25, "pm25.json", decode_pm25_response, Pm25OperationResponse::Ok(_));
sample_deserializes!(
    air_temperature,
    "air-temperature.json",
    decode_air_temperature_response,
    AirTemperatureOperationResponse::Ok(_)
);
sample_deserializes!(
    relative_humidity,
    "relative-humidity.json",
    decode_relative_humidity_response,
    RelativeHumidityOperationResponse::Ok(_)
);
sample_deserializes!(
    wind_speed,
    "wind-speed.json",
    decode_wind_speed_response,
    WindSpeedOperationResponse::Ok(_)
);
sample_deserializes!(
    wind_direction,
    "wind-direction.json",
    decode_wind_direction_response,
    WindDirectionOperationResponse::Ok(_)
);
sample_deserializes!(
    rainfall,
    "rainfall.json",
    decode_rainfall_response,
    RainfallOperationResponse::Ok(_)
);
sample_deserializes!(
    two_hr_forecast,
    "two-hr-forecast.json",
    decode_two_hr_forecast_response,
    TwoHrForecastOperationResponse::Ok(_)
);
sample_deserializes!(
    twenty_four_hr_forecast,
    "twenty-four-hr-forecast.json",
    decode_twenty_four_hr_forecast_response,
    TwentyFourHrForecastOperationResponse::Ok(_)
);
sample_deserializes!(
    four_day_outlook,
    "four-day-outlook.json",
    decode_four_day_outlook_response,
    FourDayOutlookOperationResponse::Ok(_)
);
sample_deserializes!(uv, "uv.json", decode_uv_response, UvOperationResponse::Ok(_));
