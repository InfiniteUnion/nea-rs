//! Sample live NEA upstream responses with the generated client.

use std::any::Any;
use std::error::Error;
use std::fmt::Debug;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, mem, panic};

use http::{HeaderMap, Method, StatusCode};
use nea_rs::Api;
use nea_rs::NeaWeatherSubApi::{Lightning, WetBulbGlobeTemperature};
use reqwest::{Client, Request, retry};
use satay_runtime::{Action, ResponseParts};
use tokio::{
    task::JoinSet,
    time::{Instant, sleep},
};

const ISSUE_REPORT_PATH: &str = "target/nea-upstream-sampler-issue.md";
const FAILURE_SAMPLES_DIR: &str = "target/nea-upstream-samples";
const RESPONSE_BODY_LIMIT: usize = 4_000;
const PROBE_STAGGER: Duration = Duration::from_secs(3);
const RETRY_DELAYS: [Duration; 2] = [Duration::from_secs(1), Duration::from_secs(2)];

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

#[derive(Debug)]
struct AttemptError<E> {
    error: E,
    retryable: bool,
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
    if let Err(error) = remove_stale_artifacts() {
        eprintln!("failed to remove stale sampler artifacts: {error}");
        return ExitCode::from(1);
    }

    let builder = Client::builder().timeout(Duration::from_secs(30));
    let client = match builder.retry(retry::never()).build() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("failed to build HTTP client: {error}");
            return ExitCode::from(1);
        }
    };
    let api = api_from_env();

    let mut tasks = JoinSet::new();
    spawn_probe::<0, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("psi", api.air_quality().psi(), &client).await
    });
    spawn_probe::<1, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("pm25", api.air_quality().pm25(), &client).await
    });
    spawn_probe::<2, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "air_temperature",
            api.weather_readings().air_temperature(),
            &client,
        )
        .await
    });
    spawn_probe::<3, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "relative_humidity",
            api.weather_readings().relative_humidity(),
            &client,
        )
        .await
    });
    spawn_probe::<4, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("wind_speed", api.weather_readings().wind_speed(), &client).await
    });
    spawn_probe::<5, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "wind_direction",
            api.weather_readings().wind_direction(),
            &client,
        )
        .await
    });
    spawn_probe::<6, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("rainfall", api.weather_readings().rainfall(), &client).await
    });
    spawn_probe::<7, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "two_hr_forecast",
            api.weather_forecast().two_hr_forecast(),
            &client,
        )
        .await
    });
    spawn_probe::<8, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "twenty_four_hr_forecast",
            api.weather_forecast().twenty_four_hr_forecast(),
            &client,
        )
        .await
    });
    spawn_probe::<9, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "four_day_outlook",
            api.weather_forecast().four_day_outlook(),
            &client,
        )
        .await
    });
    spawn_probe::<10, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action("uv", api.environment().uv(), &client).await
    });
    spawn_probe::<11, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "weather_lightning",
            api.environment().weather_sub_api(Lightning),
            &client,
        )
        .await
    });
    spawn_probe::<12, _, _>(&mut tasks, &api, &client, |api, client| async move {
        probe_action(
            "weather_wbgt",
            api.environment().weather_sub_api(WetBulbGlobeTemperature),
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
    let mut client_failures = vec![];
    let mut transport_failures = vec![];
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

    let sample_count = match write_failure_samples(&client_failures) {
        Ok(sample_count) => sample_count,
        Err(error) => {
            eprintln!("failed to write sampler regression samples: {error}");
            return ExitCode::from(1);
        }
    };

    println!(
        "wrote sampler issue report to {ISSUE_REPORT_PATH} for {} generated-client failure(s) \
         and {sample_count} regression sample candidate(s)",
        client_failures.len(),
    );
    ExitCode::from(2)
}

fn remove_stale_artifacts() -> io::Result<()> {
    match fs::remove_file(ISSUE_REPORT_PATH) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    match fs::remove_dir_all(FAILURE_SAMPLES_DIR) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    Ok(())
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

fn write_failure_samples(failures: &[ProbeFailure]) -> io::Result<usize> {
    let candidates = failures
        .iter()
        .filter(|failure| is_failure_sample_candidate(failure))
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        return Ok(0);
    }

    fs::create_dir_all(FAILURE_SAMPLES_DIR)?;

    for failure in &candidates {
        let file_name = format!("{}.json", failure.endpoint.replace('_', "-"));
        fs::write(
            Path::new(FAILURE_SAMPLES_DIR).join(file_name),
            &failure.body,
        )?;
    }

    Ok(candidates.len())
}

fn is_failure_sample_candidate(failure: &ProbeFailure) -> bool {
    failure.status == StatusCode::OK
        && serde_json::from_slice::<serde_json::Value>(&failure.body).is_ok()
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

async fn probe_action<'de, A>(endpoint: &'static str, action: A, client: &Client) -> ProbeOutcome
where
    A: Action + Send,
    A::RequestBody: Into<reqwest::Body>,
    A::Response<'de>: Debug,
{
    probe_action_with_transport(endpoint, action, |request| execute_request(client, request)).await
}

async fn execute_request(
    client: &Client,
    request: Request,
) -> Result<ResponseParts<impl AsRef<[u8]>>, AttemptError<reqwest::Error>> {
    let mut response = client
        .execute(request)
        .await
        .map_err(classify_transport_error)?;
    let status = response.status();
    let headers = mem::take(response.headers_mut());
    let body = response.bytes().await.map_err(classify_transport_error)?;
    Ok(ResponseParts {
        status,
        headers,
        body,
    })
}

fn classify_transport_error(error: reqwest::Error) -> AttemptError<reqwest::Error> {
    let retryable = !error.is_builder()
        && !error.is_redirect()
        && !error.is_status()
        && (error.is_timeout() || error.is_connect() || has_transient_source(&error));
    AttemptError { error, retryable }
}

fn has_transient_source(mut error: &(dyn Error + 'static)) -> bool {
    loop {
        if let Some(error) = error.downcast_ref::<io::Error>()
            && matches!(
                error.kind(),
                ErrorKind::TimedOut
                    | ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
                    | ErrorKind::UnexpectedEof
            )
        {
            return true;
        }

        if let Some(error) = error.downcast_ref::<hyper::Error>()
            && (error.is_incomplete_message() || error.is_closed())
        {
            return true;
        }

        match error.source() {
            Some(source) => error = source,
            None => return false,
        }
    }
}

async fn execute_with_retry<F, Fut, B, E>(
    endpoint: &'static str,
    mut request: Request,
    mut execute: F,
) -> Result<ResponseParts<B>, AttemptError<E>>
where
    F: FnMut(Request) -> Fut,
    Fut: Future<Output = Result<ResponseParts<B>, AttemptError<E>>>,
    E: Debug,
{
    let started = Instant::now();
    let mut attempt = 0;

    loop {
        // Sending consumes the request; retain a replay only while another GET attempt is possible.
        let replay = if request.method() == Method::GET && attempt < RETRY_DELAYS.len() {
            request.try_clone()
        } else {
            None
        };
        let attempt_started = Instant::now();

        match execute(request).await {
            Ok(response) => {
                if attempt > 0 {
                    eprintln!(
                        "transport recovered: {endpoint}: {} attempts in {:?}",
                        attempt + 1,
                        started.elapsed(),
                    );
                }

                return Ok(response);
            }
            Err(error) => {
                let replay = replay.filter(|_| error.retryable);
                eprintln!(
                    "transport attempt failed: {endpoint}: attempt {}/{} after {:?}: \
                     retry={}: {:?}",
                    attempt + 1,
                    RETRY_DELAYS.len() + 1,
                    attempt_started.elapsed(),
                    replay.is_some(),
                    error.error,
                );

                let Some(next_request) = replay else {
                    return Err(error);
                };

                sleep(RETRY_DELAYS[attempt]).await;
                attempt += 1;
                request = next_request;
            }
        }
    }
}

async fn probe_action_with_transport<'de, A, F, Fut, B, E>(
    endpoint: &'static str,
    action: A,
    execute: F,
) -> ProbeOutcome
where
    A: Action + Send,
    A::RequestBody: Into<reqwest::Body>,
    A::Response<'de>: Debug,
    F: FnMut(Request) -> Fut,
    Fut: Future<Output = Result<ResponseParts<B>, AttemptError<E>>>,
    B: AsRef<[u8]>,
    E: Debug,
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

    let response = match execute_with_retry(endpoint, reqwest_request, execute).await {
        Ok(response) => response,
        Err(error) => {
            return ProbeOutcome::TransportFailure(TransportFailure {
                endpoint,
                method: Some(method),
                uri: Some(uri),
                error: format!("{:?}", error.error),
            });
        }
    };

    classify_decode::<A>(
        endpoint,
        method,
        uri,
        response.status,
        response.headers,
        response.body.as_ref(),
    )
}

fn classify_decode<'de, A>(
    endpoint: &'static str,
    method: String,
    uri: String,
    status: StatusCode,
    headers: HeaderMap,
    body: &[u8],
) -> ProbeOutcome
where
    A: Action,
    A::Response<'de>: Debug,
{
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        A::decode(ResponseParts {
            status,
            headers,
            body,
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
            body: body.to_owned(),
        }),
        Err(payload) => ProbeOutcome::ClientFailure(ProbeFailure {
            endpoint,
            method,
            uri,
            status,
            failure_kind: "panic",
            error: panic_message(payload.as_ref()),
            body: body.to_owned(),
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
#[path = "upstream_sampler/tests.rs"]
mod tests;
