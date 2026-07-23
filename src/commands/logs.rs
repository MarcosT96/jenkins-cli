//! Console log command.
//!
//! Without `--follow`: fetch the full log via `consoleText`. With `--follow`:
//! poll `logText/progressiveText?start={offset}`, printing only the new text
//! each round and using the `X-Text-Size`/`X-More-Data` response headers to
//! know the next offset and whether the build is still running.

use std::thread::sleep;
use std::time::Duration;

use crate::cli::{GlobalArgs, LogsArgs};
use crate::client::Client;
use crate::error::{AppError, Result};

pub fn run(args: LogsArgs, _global: &GlobalArgs) -> Result<()> {
    let client = Client::new()?;
    let build_path = build_path(&args.job, args.build_number);

    if args.follow {
        follow(&client, &args.job, &build_path, args.poll)
    } else {
        let resp = client
            .get_raw(&format!("{build_path}consoleText"))
            .map_err(|e| job_not_found(e, &args.job))?;
        print!("{}", resp.body);
        Ok(())
    }
}

pub fn build_path(job: &str, build_number: Option<u64>) -> String {
    match build_number {
        Some(n) => format!("/job/{job}/{n}/"),
        None => format!("/job/{job}/lastBuild/"),
    }
}

fn follow(client: &Client, job: &str, build_path: &str, poll_secs: u64) -> Result<()> {
    let mut start: u64 = 0;

    loop {
        let resp = client
            .get_raw(&format!(
                "{build_path}logText/progressiveText?start={start}"
            ))
            .map_err(|e| job_not_found(e, job))?;

        print!("{}", resp.body);

        let next_start = resp
            .headers
            .get("X-Text-Size")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(start);

        let more_data = resp
            .headers
            .get("X-More-Data")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        start = next_start;

        if !more_data {
            return Ok(());
        }
        sleep(Duration::from_secs(poll_secs));
    }
}

pub fn job_not_found(err: AppError, job: &str) -> AppError {
    match err {
        AppError::Status(404, _) => AppError::JobNotFound(job.to_string()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn client(server: &MockServer) -> Client {
        Client::with_base(&server.base_url(), "me", "tok", false).unwrap()
    }

    #[test]
    fn follow_stops_when_no_more_data() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/job/my-job/lastBuild/logText/progressiveText")
                .query_param("start", "0");
            then.status(200)
                .header("X-Text-Size", "11")
                .header("X-More-Data", "false")
                .body("hello world");
        });

        follow(&client(&server), "my-job", "/job/my-job/lastBuild/", 0).unwrap();
    }

    #[test]
    fn follow_polls_until_more_data_is_false() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/job/my-job/lastBuild/logText/progressiveText")
                .query_param("start", "0");
            then.status(200)
                .header("X-Text-Size", "5")
                .header("X-More-Data", "true")
                .body("part1");
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/job/my-job/lastBuild/logText/progressiveText")
                .query_param("start", "5");
            then.status(200)
                .header("X-Text-Size", "10")
                .header("X-More-Data", "false")
                .body("part2");
        });

        follow(&client(&server), "my-job", "/job/my-job/lastBuild/", 0).unwrap();
    }

    #[test]
    fn missing_job_maps_to_job_not_found() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/job/missing/lastBuild/consoleText");
            then.status(404);
        });
        let err = client(&server)
            .get_raw("/job/missing/lastBuild/consoleText")
            .map_err(|e| job_not_found(e, "missing"))
            .unwrap_err();
        assert!(matches!(err, AppError::JobNotFound(job) if job == "missing"));
    }
}
