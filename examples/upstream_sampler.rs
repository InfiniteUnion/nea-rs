//! Sample live NEA upstream responses with the generated client.

use std::any::Any;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, mem, panic};

use http::{HeaderMap, StatusCode};
use nea_rs::Api;
use nea_rs::NeaWeatherSubApi::{Lightning, WetBulbGlobeTemperature};
use reqwest::{Client, Request};
use satay_runtime::{Action, ResponseParts};
use tokio::{task::JoinSet, time::sleep};

const ISSUE_REPORT_PATH: &str = "target/nea-upstream-sampler-issue.md";
const RESPONSE_BODY_LIMIT: usize = 4_000;
const PROBE_STAGGER: Duration = Duration::from_secs(3);

#[derive(Debug)]
enum ProbeOutcome {
    Success,
    ClientFailure(ProbeFailure),
    TransportFailure(TransportFailure),
}

#[derive(Debug)]
struct ProbeTaskResult {
    slot: u32,
    outcome: ProbeOutcome,
}

#[derive(Debug)]
struct ProbeFailure {
    endpoint: &'static str,
    method: String,
    uri: String,
    status: StatusCode,
    failure_kind: &'static str,
    error: String,
    body: Vec<u8>,
}

#[derive(Debug)]
struct TransportFailure {
    endpoint: &'static str,
    method: Option<String>,
    uri: Option<String>,
    error: String,
}

fn spawn_probe<const SLOT: u32, F, Fut>(
    tasks: &mut JoinSet<ProbeTaskResult>,
    api: &Api,
    client: &Client,
    probe: F,
) where
    F: FnOnce(Api, Client) -> Fut + Send + 'static,
    Fut: Future<Output = ProbeOutcome> + Send + 'static,
{
    let api = api.clone();
    let client = client.clone();
    tasks.spawn(async move {
        sleep(PROBE_STAGGER * SLOT).await;
        let outcome = probe(api, client).await;
        ProbeTaskResult {
            slot: SLOT,
            outcome,
        }
    });
}

#[tokio::main]
async fn main() -> ExitCode {
    run_sampler().await
}

async fn run_sampler() -> ExitCode {
    if let Err(error) = remove_stale_issue_report() {
        eprintln!("failed to remove stale sampler issue report: {error}");
        return ExitCode::from(1);
    }

    let client = match Client::builder().timeout(Duration::from_secs(30)).build() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to build HTTP client: {error}");
            return ExitCode::from(1);
        }
    };
    let api = api_from_env();

    let mut tasks = JoinSet::new();
    spawn_probe::<0, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("psi", api.psi(), &client).await
    });
    spawn_probe::<1, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("pm25", api.pm25(), &client).await
    });
    spawn_probe::<2, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("air_temperature", api.air_temperature(), &client).await
    });
    spawn_probe::<3, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("relative_humidity", api.relative_humidity(), &client).await
    });
    spawn_probe::<4, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("wind_speed", api.wind_speed(), &client).await
    });
    spawn_probe::<5, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("wind_direction", api.wind_direction(), &client).await
    });
    spawn_probe::<6, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("rainfall", api.rainfall(), &client).await
    });
    spawn_probe::<7, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("two_hr_forecast", api.two_hr_forecast(), &client).await
    });
    spawn_probe::<8, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "twenty_four_hr_forecast",
            api.twenty_four_hr_forecast(),
            &client,
        )
        .await
    });
    spawn_probe::<9, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("four_day_outlook", api.four_day_outlook(), &client).await
    });
    spawn_probe::<10, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("uv", api.uv(), &client).await
    });
    spawn_probe::<11, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("weather_lightning", api.weather_sub_api(Lightning), &client).await
    });
    spawn_probe::<12, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "weather_wbgt",
            api.weather_sub_api(WetBulbGlobeTemperature),
            &client,
        )
        .await
    });

    let mut results = collect_task_results(tasks).await;
    results.sort_by_key(|result| result.slot);

    finish_run(results)
}

async fn collect_task_results(mut tasks: JoinSet<ProbeTaskResult>) -> Vec<ProbeTaskResult> {
    let mut results = vec![];
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(result) => results.push(result),
            Err(error) => results.push(ProbeTaskResult {
                slot: u32::MAX,
                outcome: ProbeOutcome::TransportFailure(TransportFailure {
                    endpoint: "sampler_task",
                    method: None,
                    uri: None,
                    error: error.to_string(),
                }),
            }),
        }
    }
    results
}

fn finish_run(results: Vec<ProbeTaskResult>) -> ExitCode {
    let mut client_failures = Vec::new();
    let mut transport_failures = Vec::new();
    let mut success_count = 0_usize;

    for result in results {
        match result.outcome {
            ProbeOutcome::Success => success_count += 1,
            ProbeOutcome::ClientFailure(failure) => client_failures.push(failure),
            ProbeOutcome::TransportFailure(failure) => transport_failures.push(failure),
        }
    }

    for failure in &transport_failures {
        eprintln!("transport failure: {}: {}", failure.endpoint, failure.error);
    }

    if client_failures.is_empty() && transport_failures.is_empty() {
        println!("all {success_count} upstream probes succeeded");
        return ExitCode::SUCCESS;
    }

    if client_failures.is_empty() {
        return ExitCode::from(1);
    }

    if let Err(error) = write_issue_report(&client_failures, &transport_failures) {
        eprintln!("failed to write sampler issue report: {error}");
        return ExitCode::from(1);
    }

    println!(
        "wrote sampler issue report to {ISSUE_REPORT_PATH} for {} generated-client failure(s)",
        client_failures.len()
    );
    ExitCode::from(2)
}

fn remove_stale_issue_report() -> io::Result<()> {
    match fs::remove_file(ISSUE_REPORT_PATH) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_issue_report(
    failures: &[ProbeFailure],
    transport_failures: &[TransportFailure],
) -> io::Result<()> {
    fs::create_dir_all("target")?;
    fs::write(
        ISSUE_REPORT_PATH,
        render_issue_report(failures, transport_failures),
    )
}

fn api_from_env() -> Api {
    api_from_optional_key(env::var("X_API_KEY").ok())
}

fn api_from_optional_key(key: Option<String>) -> Api {
    let api = Api::new();
    if let Some(key) = key.filter(|key| !key.trim().is_empty()) {
        api.x_api_key(key)
    } else {
        api
    }
}

async fn probe_action<A>(endpoint: &'static str, action: A, client: &Client) -> ProbeOutcome
where
    A: Action + Send,
    A::Response: Debug,
{
    let http_request = match action.request() {
        Ok(request) => request,
        Err(error) => {
            return ProbeOutcome::TransportFailure(TransportFailure {
                endpoint,
                method: None,
                uri: None,
                error: error.to_string(),
            });
        }
    };

    let method = http_request.method().to_string();
    let uri = http_request.uri().to_string();
    let reqwest_request: Request = match http_request.try_into() {
        Ok(request) => request,
        Err(error) => {
            return ProbeOutcome::TransportFailure(TransportFailure {
                endpoint,
                method: Some(method),
                uri: Some(uri),
                error: error.to_string(),
            });
        }
    };

    let mut response = match client.execute(reqwest_request).await {
        Ok(response) => response,
        Err(error) => {
            return ProbeOutcome::TransportFailure(TransportFailure {
                endpoint,
                method: Some(method),
                uri: Some(uri),
                error: error.to_string(),
            });
        }
    };

    let status = response.status();
    let headers = mem::take(response.headers_mut());
    let body = match response.bytes().await {
        Ok(body) => body,
        Err(error) => {
            return ProbeOutcome::TransportFailure(TransportFailure {
                endpoint,
                method: Some(method),
                uri: Some(uri),
                error: error.to_string(),
            });
        }
    };

    classify_decode::<A>(endpoint, method, uri, status, headers, body.to_vec())
}

fn classify_decode<A>(
    endpoint: &'static str,
    method: String,
    uri: String,
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
) -> ProbeOutcome
where
    A: Action,
    A::Response: Debug,
{
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        A::decode(ResponseParts {
            status,
            headers,
            body: body.as_slice(),
        })
    }));

    match result {
        Ok(Ok(response)) => {
            let _ = response;
            ProbeOutcome::Success
        }
        Ok(Err(error)) => ProbeOutcome::ClientFailure(ProbeFailure {
            endpoint,
            method,
            uri,
            status,
            failure_kind: "decode error",
            error: error.to_string(),
            body,
        }),
        Err(payload) => ProbeOutcome::ClientFailure(ProbeFailure {
            endpoint,
            method,
            uri,
            status,
            failure_kind: "panic",
            error: panic_message(payload.as_ref()),
            body,
        }),
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

fn render_issue_report(
    failures: &[ProbeFailure],
    transport_failures: &[TransportFailure],
) -> String {
    let mut report = String::new();
    report.push_str("# NEA upstream API sample decode failure\n\n");
    report.push_str(
        "The generated nea-rs client failed while decoding live upstream response(s).\n\n",
    );
    report.push_str("- commit: ");
    report.push_str(&env::var("GITHUB_SHA").unwrap_or_else(|_| "unknown".to_owned()));
    report.push_str("\n- run: ");
    report.push_str(&github_run_url().unwrap_or_else(|| "unknown".to_owned()));
    report.push('\n');

    for failure in failures {
        render_probe_failure(&mut report, failure);
    }

    if !transport_failures.is_empty() {
        report.push_str("\n## Transport failures in same run\n\n");
        for failure in transport_failures {
            report.push_str("- `");
            report.push_str(failure.endpoint);
            report.push('`');
            if let (Some(method), Some(uri)) = (&failure.method, &failure.uri) {
                report.push_str(": `");
                report.push_str(method);
                report.push(' ');
                report.push_str(uri);
                report.push('`');
            }
            report.push_str(": ");
            report.push_str(&failure.error);
            report.push('\n');
        }
    }

    report
}

fn render_probe_failure(report: &mut String, failure: &ProbeFailure) {
    report.push_str("\n## Endpoint `");
    report.push_str(failure.endpoint);
    report.push_str("`\n\n");
    report.push_str("- request: `");
    report.push_str(&failure.method);
    report.push(' ');
    report.push_str(&failure.uri);
    report.push_str("`\n");
    report.push_str("- status: `");
    report.push_str(&failure.status.as_u16().to_string());
    report.push(' ');
    report.push_str(failure.status.canonical_reason().unwrap_or_default());
    report.push_str("`\n");
    report.push_str("- failure: `");
    report.push_str(failure.failure_kind);
    report.push_str(": ");
    report.push_str(&failure.error);
    report.push_str("`\n");
    report.push_str("- response body (UTF-8 lossy, first 4000 bytes):\n\n````text\n");
    report.push_str(&body_excerpt(&failure.body));
    report.push_str("\n````\n");
    if failure.body.len() > RESPONSE_BODY_LIMIT {
        report.push_str("\n_response body truncated from ");
        report.push_str(&failure.body.len().to_string());
        report.push_str(" bytes to 4000 bytes_\n");
    }
}

fn body_excerpt(body: &[u8]) -> String {
    String::from_utf8_lossy(&body[..body.len().min(RESPONSE_BODY_LIMIT)]).into_owned()
}

fn github_run_url() -> Option<String> {
    let server_url = env::var("GITHUB_SERVER_URL").ok()?;
    let repository = env::var("GITHUB_REPOSITORY").ok()?;
    let run_id = env::var("GITHUB_RUN_ID").ok()?;
    Some(format!("{server_url}/{repository}/actions/runs/{run_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_api_key_is_not_sent() {
        let request = api_from_optional_key(Some(String::new()))
            .air_temperature()
            .request()
            .expect("air temperature request should build");
        assert!(request.headers().get("x-api-key").is_none());

        let request = api_from_optional_key(Some("test-key".to_owned()))
            .air_temperature()
            .request()
            .expect("air temperature request should build");
        assert_eq!(
            request.headers().get("x-api-key"),
            Some(&"test-key".parse().unwrap())
        );
    }

    #[test]
    fn render_issue_report_includes_response_body() {
        let failure = ProbeFailure {
            endpoint: "air_temperature",
            method: "GET".to_owned(),
            uri: "https://api-open.data.gov.sg/v2/real-time/api/air-temperature".to_owned(),
            status: StatusCode::OK,
            failure_kind: "decode error",
            error: "JSON error: missing field data".to_owned(),
            body: br#"{"unexpected":true}"#.to_vec(),
        };

        let report = render_issue_report(&[failure], &[]);

        assert!(report.contains("## Endpoint `air_temperature`"));
        assert!(report.contains("JSON error: missing field data"));
        assert!(report.contains(r#"{"unexpected":true}"#));
    }

    #[test]
    fn body_excerpt_truncates_large_response() {
        let body = vec![b'a'; RESPONSE_BODY_LIMIT + 1];
        let excerpt = body_excerpt(&body);
        assert_eq!(excerpt.len(), RESPONSE_BODY_LIMIT);
        assert!(excerpt.bytes().all(|byte| byte == b'a'));

        let failure = ProbeFailure {
            endpoint: "air_temperature",
            method: "GET".to_owned(),
            uri: "https://api-open.data.gov.sg/v2/real-time/api/air-temperature".to_owned(),
            status: StatusCode::OK,
            failure_kind: "decode error",
            error: "JSON error: missing field data".to_owned(),
            body,
        };
        let report = render_issue_report(&[failure], &[]);
        assert!(report.contains("truncated from 4001 bytes to 4000 bytes"));
    }

    #[test]
    fn decode_panic_becomes_client_failure() {
        struct PanicAction;

        impl Action for PanicAction {
            type Response = ();

            fn request(self) -> Result<http::Request<Vec<u8>>, satay_runtime::Error> {
                unreachable!("request is not used by decode classification test")
            }

            fn decode<B: AsRef<[u8]>>(
                _: ResponseParts<B>,
            ) -> Result<Self::Response, satay_runtime::Error> {
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
            body.clone(),
        );

        let ProbeOutcome::ClientFailure(failure) = outcome else {
            panic!("panic should be classified as a generated-client failure");
        };
        assert_eq!(failure.failure_kind, "panic");
        assert_eq!(failure.error, "decode exploded");
        assert_eq!(failure.body, body);
    }
}
