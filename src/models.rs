//! Typed Jenkins REST API response shapes.

use serde::Deserialize;

/// A queue item, returned while polling `{queueUrl}api/json` after triggering
/// a build. `executable` is `None` until Jenkins assigns an executor.
#[derive(Debug, Deserialize)]
pub struct QueueItem {
    #[serde(default)]
    pub cancelled: bool,
    pub executable: Option<Executable>,
    pub why: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Executable {
    pub number: u64,
    pub url: String,
}

/// A build's status, from `/job/{job}/{n}/api/json` or `/lastBuild/api/json`.
#[derive(Debug, Deserialize)]
pub struct BuildInfo {
    pub building: bool,
    pub result: Option<String>,
    pub number: u64,
    pub url: String,
    #[serde(default)]
    pub duration: i64,
    #[serde(rename = "estimatedDuration", default)]
    pub estimated_duration: i64,
}

#[derive(Debug, Deserialize)]
pub struct JobSummary {
    pub name: String,
    pub color: Option<String>,
    pub url: String,
}
