//! Build trigger command.
//!
//! `POST /job/{job}/build` (or `buildWithParameters` when `--param` is given)
//! returns no useful body — the queue item location is in the `Location`
//! response header. With `--wait`, polls the queue item until Jenkins assigns
//! an executor, then polls the build itself until it stops running.

use std::thread::sleep;
use std::time::Duration;

use crate::cli::{BuildArgs, GlobalArgs};
use crate::client::Client;
use crate::error::{AppError, Result};
use crate::models::{BuildInfo, QueueItem};
use crate::output;

pub fn run(args: BuildArgs, global: &GlobalArgs) -> Result<()> {
    trigger(
        &args.job,
        &args.params,
        args.wait,
        args.timeout,
        args.poll,
        global,
    )
}

fn trigger(
    job: &str,
    params: &[String],
    wait: bool,
    timeout: u64,
    poll: u64,
    global: &GlobalArgs,
) -> Result<()> {
    let client = Client::new()?;
    let queue_url = trigger_build(&client, job, params)?;

    if !wait {
        if global.json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({ "job": job, "queueUrl": queue_url, "status": "queued" })
                )?
            );
        } else {
            output::line(&format!("Queued: {queue_url}"), "green");
        }
        return Ok(());
    }

    let build = wait_for_build(&client, &queue_url, timeout, poll)?;
    print_build(job, &build, global)
}

/// Trigger a build, returning the queue item URL from the `Location` header.
pub fn trigger_build(client: &Client, job: &str, params: &[String]) -> Result<String> {
    let resp = if params.is_empty() {
        client.post(&format!("/job/{job}/build"), None)
    } else {
        let form = parse_params(params)?;
        client.post(&format!("/job/{job}/buildWithParameters"), Some(&form))
    };

    let resp = match resp {
        Err(AppError::Status(404, _)) => return Err(AppError::JobNotFound(job.to_string())),
        other => other?,
    };

    resp.headers
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Api("Jenkins did not return a queue URL".into()))
}

fn parse_params(params: &[String]) -> Result<Vec<(String, String)>> {
    params
        .iter()
        .map(|p| {
            p.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .ok_or_else(|| {
                    AppError::Usage(format!("invalid --param \"{p}\" (expected key=value)"))
                })
        })
        .collect()
}

/// Poll the queue item until Jenkins assigns a build, then poll the build
/// until it stops running. Returns the final build info.
pub fn wait_for_build(
    client: &Client,
    queue_url: &str,
    timeout_secs: u64,
    poll_secs: u64,
) -> Result<BuildInfo> {
    let max_polls = (timeout_secs / poll_secs.max(1)).max(1);

    let mut build_url = None;
    for _ in 0..max_polls {
        let item: QueueItem = client.get_json_absolute(&queue_item_api(queue_url))?;
        if item.cancelled {
            return Err(AppError::QueueCancelled(
                item.why.unwrap_or_else(|| queue_url.to_string()),
            ));
        }
        if let Some(exec) = item.executable {
            build_url = Some(exec.url);
            break;
        }
        output::inline(".", "yellow");
        sleep(Duration::from_secs(poll_secs));
    }

    let build_url =
        build_url.ok_or_else(|| AppError::WaitTimeout(format!("Still queued: {queue_url}")))?;

    for _ in 0..max_polls {
        let info: BuildInfo = client.get_json_absolute(&build_api(&build_url))?;
        if !info.building {
            output::line("", "white");
            return Ok(info);
        }
        output::inline(".", "yellow");
        sleep(Duration::from_secs(poll_secs));
    }

    Err(AppError::WaitTimeout(format!(
        "Still building: {build_url}"
    )))
}

fn queue_item_api(url: &str) -> String {
    api_path(url)
}

fn build_api(url: &str) -> String {
    api_path(url)
}

fn api_path(url: &str) -> String {
    if url.ends_with('/') {
        format!("{url}api/json")
    } else {
        format!("{url}/api/json")
    }
}

fn print_build(job: &str, build: &BuildInfo, global: &GlobalArgs) -> Result<()> {
    let result = build
        .result
        .clone()
        .unwrap_or_else(|| "FAILURE".to_string());
    let value = serde_json::json!({
        "job": job,
        "buildNumber": build.number,
        "url": build.url,
        "result": result,
        "durationSecs": build.duration / 1000,
    });

    if global.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        output::print_value(&value);
    }

    if result != "SUCCESS" {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    fn client(server: &MockServer) -> Client {
        Client::with_base(&server.base_url(), "me", "tok", false).unwrap()
    }

    #[test]
    fn trigger_build_without_params() {
        let server = MockServer::start();
        let location = format!("{}/queue/item/9/", server.base_url());
        let mock = server.mock(|when, then| {
            when.method(POST).path("/job/my-job/build");
            then.status(201).header("Location", &location);
        });
        let url = trigger_build(&client(&server), "my-job", &[]).unwrap();
        mock.assert();
        assert_eq!(url, location);
    }

    #[test]
    fn trigger_build_with_params_uses_build_with_parameters() {
        let server = MockServer::start();
        let location = format!("{}/queue/item/10/", server.base_url());
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/job/my-job/buildWithParameters")
                .body("branch=main");
            then.status(201).header("Location", &location);
        });
        let url = trigger_build(&client(&server), "my-job", &["branch=main".to_string()]).unwrap();
        mock.assert();
        assert_eq!(url, location);
    }

    #[test]
    fn trigger_build_missing_job_maps_to_job_not_found() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/job/missing/build");
            then.status(404);
        });
        let err = trigger_build(&client(&server), "missing", &[]).unwrap_err();
        assert!(matches!(err, AppError::JobNotFound(job) if job == "missing"));
    }

    #[test]
    fn trigger_build_without_location_header_is_api_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/job/my-job/build");
            then.status(201);
        });
        let err = trigger_build(&client(&server), "my-job", &[]).unwrap_err();
        assert!(matches!(err, AppError::Api(_)));
    }

    #[test]
    fn wait_for_build_follows_queue_then_build() {
        let server = MockServer::start();
        let queue_url = format!("{}/queue/item/1/", server.base_url());
        let build_url = format!("{}/job/my-job/5/", server.base_url());

        server.mock(|when, then| {
            when.method(GET).path("/queue/item/1/api/json");
            then.status(200).json_body(json!({
                "cancelled": false,
                "executable": { "number": 5, "url": build_url },
                "why": null,
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/job/my-job/5/api/json");
            then.status(200).json_body(json!({
                "building": false,
                "result": "SUCCESS",
                "number": 5,
                "url": build_url,
                "duration": 2000,
                "estimatedDuration": 2000,
            }));
        });

        let build = wait_for_build(&client(&server), &queue_url, 30, 0).unwrap();
        assert_eq!(build.number, 5);
        assert_eq!(build.result.as_deref(), Some("SUCCESS"));
    }

    #[test]
    fn wait_for_build_reports_cancelled_queue_item() {
        let server = MockServer::start();
        let queue_url = format!("{}/queue/item/2/", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/queue/item/2/api/json");
            then.status(200).json_body(json!({
                "cancelled": true,
                "executable": null,
                "why": "build was aborted",
            }));
        });
        let err = wait_for_build(&client(&server), &queue_url, 30, 0).unwrap_err();
        assert!(matches!(err, AppError::QueueCancelled(reason) if reason == "build was aborted"));
    }

    #[test]
    fn wait_for_build_times_out_while_queued() {
        let server = MockServer::start();
        let queue_url = format!("{}/queue/item/3/", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/queue/item/3/api/json");
            then.status(200).json_body(json!({
                "cancelled": false,
                "executable": null,
                "why": "waiting for executor",
            }));
        });
        let err = wait_for_build(&client(&server), &queue_url, 1, 1).unwrap_err();
        assert!(matches!(err, AppError::WaitTimeout(_)));
    }
}
