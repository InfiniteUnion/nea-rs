# Security Policy

## Supported Versions

Security fixes are provided for the latest published version of `nea-rs`.

Until the crate reaches `1.0`, compatibility may change between minor versions. Please test against the latest release before reporting an issue that may already be fixed.

## Reporting a Vulnerability

Please do not open a public issue for a suspected security vulnerability.

Report privately by contacting the maintainer through the repository owner account, GitHub Security Advisories (when enabled), or the contact method listed on [crates.io](https://crates.io/crates/nea-rs) once published. Include:

- affected version or commit
- a minimal reproducer (request/response bytes or a small Rust example)
- expected behavior
- observed behavior
- impact assessment

You should receive an initial response within 7 days. If the report is accepted, a fix and advisory will be coordinated before public disclosure where practical.

## About This Crate

`nea-rs` is a generated Rust client for Singapore NEA real-time environmental and weather APIs exposed via [data.gov.sg](https://data.gov.sg). It builds `http::Request` values and decodes JSON response bodies; it does not perform network I/O, TLS, or caching. Callers are responsible for fetching responses and for how credentials and URLs are chosen.

Default API base URL: `https://api-open.data.gov.sg/v2/real-time/api` (see `SERVER_URL` in the crate).

## Security Scope

Issues generally considered security-sensitive include:

- panics, aborts, or excessive CPU/memory use when parsing untrusted JSON response bodies
- unsound Rust or other memory-safety defects in this crate
- incorrect deserialization or request construction that can misrepresent readings or forecasts in a security-relevant way (for example, wrong PSI/PM2.5 values from a well-formed API response under the bundled OpenAPI spec)
- URI construction bugs when combining `base_url`, paths, and query parameters (for example, header injection or unexpected request targets when inputs are attacker-controlled)
- accidental exposure of `x-api-key` or other secrets through logging, `Debug`, or error messages emitted by this crate

Issues generally not considered vulnerabilities by themselves:

- incorrect, delayed, or unavailable data returned by NEA or data.gov.sg upstream services
- rate limiting, authentication, or quota behavior of the public API
- application-level misuse (for example, pointing `base_url` at an internal service without validating caller input—that is the integrator’s threat model)
- missing or changed upstream OpenAPI fields unless this crate mishandles responses that match the spec version shipped in `openapi/nea-realtime.openapi.yaml`
- choosing or configuring the HTTP client, connection pooling, retries, or certificate validation in downstream code

## Disclosure

Accepted vulnerabilities will be fixed in a patch release when possible. Public disclosure should include the affected versions, fixed versions, impact, and suggested mitigation.

## Generated Code

Most of this crate is generated from OpenAPI via **satay** and depends on `satay-runtime`. Reports that stem from the generator or shared runtime may be fixed here, in the generator, or in both; include which layer you believe is at fault when reporting.
