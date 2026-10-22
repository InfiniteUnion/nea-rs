use std::collections::VecDeque;
use std::future::{Ready, ready};

use super::*;

const FORECAST_BODY: &[u8] = include_bytes!("../../tests/samples/four-day-outlook-1.json");

type ScriptResult = Result<ResponseParts<&'static [u8]>, AttemptError<io::Error>>;

struct ScriptedTransport {
    steps: VecDeque<ScriptResult>,
    attempts: Vec<Instant>,
}

impl ScriptedTransport {
    fn new(steps: impl IntoIterator<Item = ScriptResult>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            attempts: vec![],
        }
    }

    fn execute(&mut self, _request: Request) -> Ready<ScriptResult> {
        self.attempts.push(Instant::now());
        ready(
            self.steps
                .pop_front()
                .expect("unexpected transport attempt"),
        )
    }
}

fn failed_attempt(kind: ErrorKind) -> ScriptResult {
    let error = io::Error::from(kind);
    Err(AttemptError {
        retryable: has_transient_source(&error),
        error,
    })
}

fn complete_response(body: &'static [u8]) -> ScriptResult {
    Ok(ResponseParts {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body,
    })
}

#[tokio::test(start_paused = true)]
async fn retries_transient_failure_before_decoding() {
    let mut transport = ScriptedTransport::new([
        failed_attempt(ErrorKind::ConnectionReset),
        complete_response(FORECAST_BODY),
    ]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new().weather_forecast().four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    assert!(matches!(outcome, ProbeOutcome::Success));
    assert_eq!(transport.attempts.len(), 2);
    assert_eq!(
        transport.attempts[1] - transport.attempts[0],
        Duration::from_secs(1),
    );
}

#[tokio::test(start_paused = true)]
async fn can_recover_on_final_attempt() {
    let mut transport = ScriptedTransport::new([
        failed_attempt(ErrorKind::TimedOut),
        failed_attempt(ErrorKind::UnexpectedEof),
        complete_response(FORECAST_BODY),
    ]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new().weather_forecast().four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    assert!(matches!(outcome, ProbeOutcome::Success));
    assert_eq!(transport.attempts.len(), 3);
    assert_eq!(
        [
            transport.attempts[1] - transport.attempts[0],
            transport.attempts[2] - transport.attempts[1],
        ],
        [Duration::from_secs(1), Duration::from_secs(2)],
    );
}

#[tokio::test(start_paused = true)]
async fn exhausted_transport_failure_fails_run() {
    let mut transport = ScriptedTransport::new([
        failed_attempt(ErrorKind::ConnectionReset),
        failed_attempt(ErrorKind::ConnectionReset),
        failed_attempt(ErrorKind::ConnectionReset),
        complete_response(FORECAST_BODY),
    ]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new().weather_forecast().four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    assert!(matches!(outcome, ProbeOutcome::TransportFailure(_)));
    assert_eq!(transport.attempts.len(), 3);
    assert_eq!(
        finish_run(vec![ProbeTaskResult { slot: 0, outcome }]),
        ExitCode::from(1),
    );
}

#[tokio::test(start_paused = true)]
async fn terminal_transport_failure_stops_immediately() {
    let mut transport = ScriptedTransport::new([
        failed_attempt(ErrorKind::InvalidData),
        complete_response(FORECAST_BODY),
    ]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new().weather_forecast().four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    assert!(matches!(outcome, ProbeOutcome::TransportFailure(_)));
    assert_eq!(transport.attempts.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn schema_failure_is_captured_without_transport_retry() {
    let mut transport = ScriptedTransport::new([
        complete_response(br#"{"unexpected":true}"#),
        complete_response(FORECAST_BODY),
    ]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new().weather_forecast().four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    let ProbeOutcome::ClientFailure(failure) = outcome else {
        panic!("incompatible JSON must remain a generated-client failure");
    };
    assert!(is_failure_sample_candidate(&failure));
    assert_eq!(transport.attempts.len(), 1);
}

#[tokio::test(start_paused = true)]
async fn invalid_request_does_not_reach_transport() {
    let mut transport = ScriptedTransport::new([]);
    let outcome = probe_action_with_transport(
        "four_day_outlook",
        Api::new()
            .x_api_key("\n".to_owned())
            .weather_forecast()
            .four_day_outlook(),
        |request| transport.execute(request),
    )
    .await;

    assert!(matches!(outcome, ProbeOutcome::TransportFailure(_)));
    assert!(transport.attempts.is_empty());
}

#[tokio::test(start_paused = true)]
async fn non_get_request_is_not_retried() {
    let mut transport = ScriptedTransport::new([
        failed_attempt(ErrorKind::ConnectionReset),
        complete_response(FORECAST_BODY),
    ]);
    let request = Request::new(
        Method::POST,
        "https://example.invalid/forecast".parse().unwrap(),
    );
    let result =
        execute_with_retry("forecast", request, |request| transport.execute(request)).await;

    assert!(result.is_err());
    assert_eq!(transport.attempts.len(), 1);
}

#[test]
fn recognizes_transient_causes_without_retrying_unknown_errors() {
    #[derive(Debug)]
    struct WrappedError(io::Error);

    impl std::fmt::Display for WrappedError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "wrapped transport error")
        }
    }

    impl Error for WrappedError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    let error = WrappedError(io::Error::from(ErrorKind::ConnectionAborted));
    assert!(has_transient_source(&error));
    assert!(!has_transient_source(&io::Error::from(ErrorKind::Other)));
}

#[test]
fn empty_api_key_is_not_sent() {
    let request = api_from_optional_key(Some(String::new()))
        .weather_readings()
        .air_temperature()
        .request()
        .expect("air temperature request should build");
    assert!(request.headers().get("x-api-key").is_none());

    let request = api_from_optional_key(Some("test-key".to_owned()))
        .weather_readings()
        .air_temperature()
        .request()
        .expect("air temperature request should build");
    assert_eq!(
        request.headers().get("x-api-key"),
        Some(&"test-key".parse().unwrap())
    );
}

#[test]
fn body_excerpt_truncates_large_response() {
    let body = vec![b'a'; RESPONSE_BODY_LIMIT + 1];
    let excerpt = body_excerpt(&body);
    assert_eq!(excerpt.len(), RESPONSE_BODY_LIMIT);
    assert!(excerpt.bytes().all(|byte| byte == b'a'));
}

#[test]
fn regression_samples_require_successful_json_responses() {
    let mut failure = ProbeFailure {
        endpoint: "air_temperature",
        method: "GET".to_owned(),
        uri: "https://api-open.data.gov.sg/v2/real-time/api/air-temperature".to_owned(),
        status: StatusCode::OK,
        failure_kind: "decode error",
        error: "JSON error: missing field data".to_owned(),
        body: br#"{"unexpected":true}"#.to_vec(),
    };

    assert!(is_failure_sample_candidate(&failure));

    failure.status = StatusCode::BAD_GATEWAY;
    assert!(!is_failure_sample_candidate(&failure));

    failure.status = StatusCode::OK;
    failure.body = b"upstream maintenance".to_vec();
    assert!(!is_failure_sample_candidate(&failure));
}

#[test]
fn decode_panic_becomes_client_failure() {
    struct PanicAction;

    impl Action for PanicAction {
        type RequestBody = Vec<u8>;

        type Response<'de> = ();

        fn request(self) -> Result<http::Request<Vec<u8>>, satay_runtime::Error> {
            unreachable!("request is not used by decode classification test")
        }

        fn decode<'de>(
            _: ResponseParts<&'de [u8]>,
        ) -> Result<Self::Response<'de>, satay_runtime::Error> {
            panic!("decode exploded")
        }
    }

    let body = br#"{"panic":true}"#.to_vec();
    let outcome = classify_decode::<PanicAction>(
        "air_temperature",
        "GET".to_owned(),
        "https://api-open.data.gov.sg/v2/real-time/api/air-temperature".to_owned(),
        StatusCode::OK,
        HeaderMap::new(),
        &body,
    );

    let ProbeOutcome::ClientFailure(failure) = outcome else {
        panic!("panic should be classified as a generated-client failure");
    };
    assert_eq!(failure.failure_kind, "panic");
    assert_eq!(failure.error, "decode exploded");
    assert_eq!(failure.body, body);
}
