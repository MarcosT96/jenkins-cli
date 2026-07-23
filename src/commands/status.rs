//! Build status command.
//!
//! Resolves status three ways: from a stored queue item URL (`--queue`), from
//! an explicit job + build number, or from a job's `lastBuild`.

use crate::cli::{GlobalArgs, StatusArgs};
use crate::client::Client;
use crate::error::{AppError, Result};
use crate::models::{BuildInfo, QueueItem};
use crate::output;

pub fn run(args: StatusArgs, global: &GlobalArgs) -> Result<()> {
    let client = Client::new()?;

    let build = if let Some(queue_url) = &args.queue {
        resolve_from_queue(&client, queue_url)?
    } else {
        match args.build_number {
            Some(n) => {
                client.job_scoped_get(&args.job, &format!("/job/{}/{}/api/json", args.job, n))?
            }
            None => client
                .job_scoped_get(&args.job, &format!("/job/{}/lastBuild/api/json", args.job))?,
        }
    };

    print_build(&args.job, &build, global)
}

/// Resolve a build from a queue item URL: if Jenkins hasn't assigned an
/// executor yet, report "pending"; otherwise fetch the build itself.
pub fn resolve_from_queue(client: &Client, queue_url: &str) -> Result<BuildInfo> {
    let item: QueueItem = client.get_json_absolute(&api_path(queue_url))?;

    if item.cancelled {
        return Err(AppError::QueueCancelled(
            item.why.unwrap_or_else(|| queue_url.to_string()),
        ));
    }

    let exec = item
        .executable
        .ok_or_else(|| AppError::Api("Build is still queued (no executor assigned yet).".into()))?;

    client.get_json_absolute(&api_path(&exec.url))
}

fn api_path(url: &str) -> String {
    if url.ends_with('/') {
        format!("{url}api/json")
    } else {
        format!("{url}/api/json")
    }
}

fn print_build(job: &str, build: &BuildInfo, global: &GlobalArgs) -> Result<()> {
    let value = serde_json::json!({
        "job": job,
        "buildNumber": build.number,
        "url": build.url,
        "building": build.building,
        "result": build.result,
        "durationSecs": build.duration / 1000,
    });

    if global.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        output::print_value(&value);
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
    fn last_build_status_via_job_scoped_get() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/job/my-job/lastBuild/api/json");
            then.status(200).json_body(json!({
                "building": false,
                "result": "SUCCESS",
                "number": 12,
                "url": "http://x/job/my-job/12/",
                "duration": 5000,
                "estimatedDuration": 5000,
            }));
        });
        let c = client(&server);
        let build: BuildInfo = c
            .job_scoped_get("my-job", "/job/my-job/lastBuild/api/json")
            .unwrap();
        assert_eq!(build.number, 12);
    }

    #[test]
    fn resolve_from_queue_pending_when_no_executable() {
        let server = MockServer::start();
        let queue_url = format!("{}/queue/item/4/", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/queue/item/4/api/json");
            then.status(200).json_body(json!({
                "cancelled": false,
                "executable": null,
                "why": "waiting",
            }));
        });
        let err = resolve_from_queue(&client(&server), &queue_url).unwrap_err();
        assert!(matches!(err, AppError::Api(_)));
    }

    #[test]
    fn resolve_from_queue_follows_executable() {
        let server = MockServer::start();
        let queue_url = format!("{}/queue/item/5/", server.base_url());
        let build_url = format!("{}/job/my-job/6/", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/queue/item/5/api/json");
            then.status(200).json_body(json!({
                "cancelled": false,
                "executable": { "number": 6, "url": build_url },
                "why": null,
            }));
        });
        server.mock(|when, then| {
            when.method(GET).path("/job/my-job/6/api/json");
            then.status(200).json_body(json!({
                "building": true,
                "result": null,
                "number": 6,
                "url": build_url,
                "duration": 0,
                "estimatedDuration": 4000,
            }));
        });
        let build = resolve_from_queue(&client(&server), &queue_url).unwrap();
        assert_eq!(build.number, 6);
        assert!(build.building);
    }
}
