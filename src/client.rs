//! Jenkins REST API client.
//!
//! Authentication is enforced by construction: a `Client` cannot be built
//! without a URL + user + API token. Basic auth with an API token does not
//! require a CSRF crumb, so this client never fetches `/crumbIssuer`.
//!
//! Status ladder:
//! * 401 -> `AppError::Unauthorized`
//! * 403 -> `AppError::Forbidden`
//! * 404 -> `AppError::JobNotFound` (job-scoped requests) or `AppError::Status`
//! * other non-2xx -> `AppError::Status(code, body)` (body carried in the
//!   error, never printed directly — printing to stdout would corrupt the MCP
//!   JSON-RPC stream)

use base64::Engine;
use reqwest::blocking::Client as HttpClient;
use reqwest::Method;
use serde::de::DeserializeOwned;
use std::time::Duration;

use crate::config;
use crate::error::{AppError, Result};

const DEFAULT_TIMEOUT_SECS: u64 = 10;

pub struct Client {
    http: HttpClient,
    auth_header: String,
    base: String,
}

/// The outcome of a request, carrying the response headers alongside the
/// body text. Needed because triggering a build returns its queue URL in the
/// `Location` header rather than the body, and progressive log tailing reads
/// `X-Text-Size` / `X-More-Data`.
#[derive(Debug)]
pub struct RawResponse {
    pub status: u16,
    pub body: String,
    pub headers: reqwest::header::HeaderMap,
}

impl Client {
    /// Build a client from saved credentials / env vars.
    pub fn new() -> Result<Self> {
        let auth = config::require_auth()?;
        let url = auth.url.unwrap_or_default();
        let user = auth.user.unwrap_or_default();
        let token = auth.api_token.unwrap_or_default();
        Self::with_base(&url, &user, &token, auth.insecure)
    }

    /// Build a client with explicit base URL and credentials. Used by tests
    /// to target a mock server; production goes through [`Client::new`].
    pub fn with_base(base: &str, user: &str, token: &str, insecure: bool) -> Result<Self> {
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{token}"));
        let http = HttpClient::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .danger_accept_invalid_certs(insecure)
            .build()?;
        Ok(Self {
            http,
            auth_header: format!("Basic {encoded}"),
            base: config::normalize_url(base),
        })
    }

    /// GET a path relative to the base URL, returning typed JSON.
    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self.request(Method::GET, path, None)?;
        Ok(serde_json::from_str(&resp.body)?)
    }

    /// GET an absolute URL (e.g. a queue item or build URL returned by
    /// Jenkins), returning typed JSON.
    pub fn get_json_absolute<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self.request_absolute(Method::GET, url, None)?;
        Ok(serde_json::from_str(&resp.body)?)
    }

    /// POST a path relative to the base URL, returning the raw response
    /// (including headers, so callers can read `Location`).
    pub fn post(&self, path: &str, form: Option<&[(String, String)]>) -> Result<RawResponse> {
        self.request_with_form(Method::POST, path, form)
    }

    /// GET raw text from a path relative to the base URL.
    pub fn get_raw(&self, path: &str) -> Result<RawResponse> {
        self.request(Method::GET, path, None)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        form: Option<&[(String, String)]>,
    ) -> Result<RawResponse> {
        self.request_with_form(method, path, form)
    }

    fn request_with_form(
        &self,
        method: Method,
        path: &str,
        form: Option<&[(String, String)]>,
    ) -> Result<RawResponse> {
        let url = format!("{}{}", self.base, path);
        self.send(method, &url, form)
    }

    fn request_absolute(
        &self,
        method: Method,
        url: &str,
        form: Option<&[(String, String)]>,
    ) -> Result<RawResponse> {
        self.send(method, url, form)
    }

    fn send(
        &self,
        method: Method,
        url: &str,
        form: Option<&[(String, String)]>,
    ) -> Result<RawResponse> {
        let mut req = self
            .http
            .request(method, url)
            .header("Authorization", &self.auth_header);

        if let Some(fields) = form {
            req = req.form(fields);
        }

        let resp = req.send()?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text()?;

        if !status.is_success() {
            match status.as_u16() {
                401 => return Err(AppError::Unauthorized),
                403 => return Err(AppError::Forbidden),
                other => return Err(AppError::Status(other, body)),
            }
        }

        Ok(RawResponse {
            status: status.as_u16(),
            body,
            headers,
        })
    }

    /// The Jenkins job-scoped 404 case is common enough to deserve a named
    /// error. Callers that expect a specific job to exist should use this
    /// wrapper instead of the raw ladder in [`Client::send`].
    pub fn job_scoped_get<T: DeserializeOwned>(&self, job: &str, path: &str) -> Result<T> {
        match self.get_json(path) {
            Err(AppError::Status(404, _)) => Err(AppError::JobNotFound(job.to_string())),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use httpmock::prelude::*;

    fn test_client(server: &MockServer) -> Client {
        Client::with_base(&server.base_url(), "me", "tok123", false).unwrap()
    }

    #[test]
    fn sends_basic_auth_header() {
        let server = MockServer::start();
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("me:tok123")
        );
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/json")
                .header("Authorization", &expected);
            then.status(200)
                .header("content-type", "application/json")
                .body("{\"mode\":\"NORMAL\"}");
        });

        let client = test_client(&server);
        let resp = client.get_raw("/api/json").unwrap();

        mock.assert();
        assert!(resp.body.contains("NORMAL"));
    }

    #[test]
    fn maps_401_to_unauthorized() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/json");
            then.status(401);
        });
        let client = test_client(&server);
        let err = client.get_raw("/api/json").unwrap_err();
        assert!(matches!(err, AppError::Unauthorized));
    }

    #[test]
    fn maps_403_to_forbidden() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/json");
            then.status(403);
        });
        let client = test_client(&server);
        let err = client.get_raw("/api/json").unwrap_err();
        assert!(matches!(err, AppError::Forbidden));
    }

    #[test]
    fn maps_404_via_job_scoped_get() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/job/missing/api/json");
            then.status(404);
        });
        let client = test_client(&server);
        let err = client
            .job_scoped_get::<crate::models::BuildInfo>("missing", "/job/missing/api/json")
            .unwrap_err();
        assert!(matches!(err, AppError::JobNotFound(job) if job == "missing"));
    }

    #[test]
    fn other_status_carries_body() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/json");
            then.status(500).body("kaboom");
        });
        let client = test_client(&server);
        let err = client.get_raw("/api/json").unwrap_err();
        match err {
            AppError::Status(code, body) => {
                assert_eq!(code, 500);
                assert!(body.contains("kaboom"));
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[test]
    fn post_build_returns_location_header() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/job/my-job/build");
            then.status(201)
                .header("Location", "http://localhost/queue/item/42/");
        });
        let client = test_client(&server);
        let resp = client.post("/job/my-job/build", None).unwrap();

        mock.assert();
        assert_eq!(resp.status, 201);
        assert_eq!(
            resp.headers.get("Location").unwrap().to_str().unwrap(),
            "http://localhost/queue/item/42/"
        );
    }

    #[test]
    fn post_build_with_parameters_form_encodes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/job/my-job/buildWithParameters")
                .body("branch=main");
            then.status(201)
                .header("Location", "http://localhost/queue/item/7/");
        });
        let client = test_client(&server);
        let form = vec![("branch".to_string(), "main".to_string())];
        let resp = client
            .post("/job/my-job/buildWithParameters", Some(&form))
            .unwrap();

        mock.assert();
        assert_eq!(resp.status, 201);
    }
}
