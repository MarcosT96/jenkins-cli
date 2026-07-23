//! MCP server (`jenkins mcp serve`) — exposes Jenkins to AI assistants.
//!
//! Reuses the existing blocking `Client` and the trigger/wait/log logic
//! already written for the CLI commands; each tool runs that blocking work
//! inside `tokio::task::spawn_blocking` since `rmcp` is async.
//!
//! Tool surface is intentionally small:
//! * `get_build_status`, `get_console_log` — read-only.
//! * `trigger_build` — DESTRUCTIVE: starts a real Jenkins build/deploy.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::Deserialize;

use crate::cli::{GlobalArgs, McpArgs, McpCmd};
use crate::client::Client;
use crate::commands::{build, logs, status};
use crate::error::{AppError, Result as AppResult};
use crate::models::BuildInfo;

pub fn run(args: McpArgs, _global: &GlobalArgs) -> AppResult<()> {
    match args.cmd.unwrap_or(McpCmd::Serve) {
        McpCmd::Serve => serve(),
    }
}

/// Spin up a tokio runtime (only for this subcommand) and serve over stdio.
fn serve() -> AppResult<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(AppError::Io)?;

    runtime.block_on(async move {
        let server = JenkinsMcp::new();
        let service = server
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await
            .map_err(|e| AppError::Api(format!("MCP serve error: {e}")))?;
        service
            .waiting()
            .await
            .map_err(|e| AppError::Api(format!("MCP wait error: {e}")))?;
        Ok::<(), AppError>(())
    })
}

#[derive(Clone)]
struct JenkinsMcp {
    // Read by the `#[tool_handler]`-generated dispatch, which the dead-code
    // analysis can't see.
    #[allow(dead_code)]
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl JenkinsMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

// ---- tool parameter types ---------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct TriggerBuildParam {
    /// Jenkins job name.
    job: String,
    /// Build parameters as key=value pairs. Triggers `buildWithParameters`
    /// when non-empty.
    #[serde(default)]
    params: Vec<String>,
    /// Wait for the build to finish before returning.
    #[serde(default)]
    wait: bool,
    /// Max seconds to wait when `wait` is true (default 1800).
    timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct BuildStatusParam {
    /// Jenkins job name.
    job: String,
    /// Build number (defaults to the last build). Ignored if `queue_url` is set.
    build_number: Option<u64>,
    /// A queue item URL returned by `trigger_build`, to resolve status of a
    /// build that may still be queued.
    queue_url: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConsoleLogParam {
    /// Jenkins job name.
    job: String,
    /// Build number (defaults to the last build).
    build_number: Option<u64>,
    /// Return only the last N lines (default 100). Set `full` to true to
    /// bypass this and return the entire log.
    tail_lines: Option<usize>,
    /// Return the full console log instead of a tail.
    #[serde(default)]
    full: bool,
}

const DEFAULT_TAIL_LINES: usize = 100;

#[tool_router]
impl JenkinsMcp {
    #[tool(
        description = "DESTRUCTIVE: trigger a Jenkins build (deploy). Confirm with the user \
                       before calling. Set wait=true to block until the build finishes and get \
                       its final result; otherwise returns immediately with a queue URL to poll \
                       via get_build_status."
    )]
    async fn trigger_build(&self, Parameters(p): Parameters<TriggerBuildParam>) -> String {
        let TriggerBuildParam {
            job,
            params,
            wait,
            timeout_secs,
        } = p;
        let timeout = timeout_secs.unwrap_or(1800);

        run_blocking(move |client| {
            let queue_url = build::trigger_build(client, &job, &params)?;
            if !wait {
                return Ok(serde_json::json!({
                    "job": job,
                    "queueUrl": queue_url,
                    "status": "queued",
                }));
            }
            let info = build::wait_for_build(client, &queue_url, timeout, 3)?;
            Ok(build_status_json(&job, &info))
        })
        .await
    }

    #[tool(
        description = "Get a build's status (read-only). Provide build_number for a specific \
                       build, queue_url to resolve a build that may still be queued, or neither \
                       for the job's last build."
    )]
    async fn get_build_status(&self, Parameters(p): Parameters<BuildStatusParam>) -> String {
        let BuildStatusParam {
            job,
            build_number,
            queue_url,
        } = p;

        run_blocking(move |client| {
            let info: BuildInfo = if let Some(url) = &queue_url {
                status::resolve_from_queue(client, url)?
            } else {
                match build_number {
                    Some(n) => client.job_scoped_get(&job, &format!("/job/{job}/{n}/api/json"))?,
                    None => {
                        client.job_scoped_get(&job, &format!("/job/{job}/lastBuild/api/json"))?
                    }
                }
            };
            Ok(build_status_json(&job, &info))
        })
        .await
    }

    #[tool(
        description = "Get a build's console log (read-only). Returns only the last tail_lines \
                       lines by default (100) to avoid flooding context — set full=true to get \
                       everything. Use this to see why a build failed."
    )]
    async fn get_console_log(&self, Parameters(p): Parameters<ConsoleLogParam>) -> String {
        let ConsoleLogParam {
            job,
            build_number,
            tail_lines,
            full,
        } = p;

        let joined = tokio::task::spawn_blocking(move || {
            let client = Client::new()?;
            let path = logs::build_path(&job, build_number);
            let resp = client
                .get_raw(&format!("{path}consoleText"))
                .map_err(|e| logs::job_not_found(e, &job))?;
            Ok::<String, AppError>(resp.body)
        })
        .await;

        let text = match joined {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return format!("Error: {e}"),
            Err(e) => return format!("Error: task failed: {e}"),
        };

        if full {
            text
        } else {
            tail(&text, tail_lines.unwrap_or(DEFAULT_TAIL_LINES))
        }
    }
}

fn build_status_json(job: &str, info: &BuildInfo) -> serde_json::Value {
    serde_json::json!({
        "job": job,
        "buildNumber": info.number,
        "url": info.url,
        "building": info.building,
        "result": info.result,
        "durationSecs": info.duration / 1000,
    })
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= n {
        text.to_string()
    } else {
        lines[lines.len() - n..].join("\n")
    }
}

/// Run a blocking closure that returns JSON, formatting the result (or error)
/// as a string for the MCP tool response.
async fn run_blocking<F>(f: F) -> String
where
    F: FnOnce(&Client) -> AppResult<serde_json::Value> + Send + 'static,
{
    let joined = tokio::task::spawn_blocking(move || {
        let client = Client::new()?;
        let value = f(&client)?;
        Ok::<String, AppError>(
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
        )
    })
    .await;
    match joined {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => format!("Error: {e}"),
        Err(e) => format!("Error: task failed: {e}"),
    }
}

#[tool_handler(
    name = "jenkins-cli",
    version = "0.1.0",
    instructions = "Jenkins tools. get_build_status and get_console_log are read-only. \
                    trigger_build starts a real Jenkins build/deploy — confirm with the user \
                    before calling it."
)]
impl ServerHandler for JenkinsMcp {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_returns_full_text_when_shorter_than_n() {
        let text = "a\nb\nc";
        assert_eq!(tail(text, 10), "a\nb\nc");
    }

    #[test]
    fn tail_returns_last_n_lines() {
        let text = "1\n2\n3\n4\n5";
        assert_eq!(tail(text, 2), "4\n5");
    }

    #[test]
    fn build_status_json_shape() {
        let info = BuildInfo {
            building: false,
            result: Some("SUCCESS".to_string()),
            number: 5,
            url: "http://x/job/my-job/5/".to_string(),
            duration: 4000,
            estimated_duration: 4000,
        };
        let value = build_status_json("my-job", &info);
        assert_eq!(value["job"], "my-job");
        assert_eq!(value["buildNumber"], 5);
        assert_eq!(value["result"], "SUCCESS");
        assert_eq!(value["durationSecs"], 4);
    }
}
