//! Job listing command.

use crate::cli::{GlobalArgs, JobArgs, JobCmd};
use crate::client::Client;
use crate::error::Result;
use crate::models::JobSummary;
use crate::output;

pub fn run(args: JobArgs, global: &GlobalArgs) -> Result<()> {
    match args.cmd.unwrap_or(JobCmd::List) {
        JobCmd::List => list(global),
    }
}

fn list(global: &GlobalArgs) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Root {
        jobs: Vec<JobSummary>,
    }

    let client = Client::new()?;
    let root: Root = client.get_json("/api/json?tree=jobs[name,color,url]")?;

    if global.json {
        let value = serde_json::to_value(
            root.jobs
                .iter()
                .map(|j| serde_json::json!({ "name": j.name, "color": j.color, "url": j.url }))
                .collect::<Vec<_>>(),
        )?;
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        for job in &root.jobs {
            output::print_value(&serde_json::json!({ "name": job.name }));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn list_deserializes_jobs() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/json");
            then.status(200).json_body(json!({
                "jobs": [
                    { "name": "my-job", "color": "blue", "url": "http://x/job/my-job/" }
                ]
            }));
        });
        let client = Client::with_base(&server.base_url(), "me", "tok", false).unwrap();

        #[derive(serde::Deserialize)]
        struct Root {
            jobs: Vec<JobSummary>,
        }
        let root: Root = client
            .get_json("/api/json?tree=jobs[name,color,url]")
            .unwrap();
        assert_eq!(root.jobs.len(), 1);
        assert_eq!(root.jobs[0].name, "my-job");
    }
}
