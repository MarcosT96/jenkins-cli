//! Command-line interface definition (clap derive).

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "jenkins", version, about = "Jenkins CLI")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

/// Flags accepted anywhere on the command line.
#[derive(Debug, Args, Clone, Default)]
pub struct GlobalArgs {
    /// Print raw JSON instead of colorized key/value output.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Authentication commands.
    Auth(AuthArgs),
    /// Trigger and watch job builds.
    Build(BuildArgs),
    /// Check build status.
    Status(StatusArgs),
    /// Job commands.
    Job(JobArgs),
    /// Run an MCP server exposing Jenkins to AI assistants (stdio).
    Mcp(McpArgs),
}

// ---- auth -------------------------------------------------------------

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub cmd: Option<AuthCmd>,
}

#[derive(Debug, Subcommand)]
pub enum AuthCmd {
    /// Save auth info (Jenkins URL + user + API token). Default action.
    Save,
    /// Show saved auth info.
    Show,
    /// Verify saved credentials against the server.
    Whoami,
}

// ---- build --------------------------------------------------------------

#[derive(Debug, Args)]
pub struct BuildArgs {
    pub job: String,
    /// Build parameter as key=value (repeatable). Implies
    /// `buildWithParameters`.
    #[arg(long = "param")]
    pub params: Vec<String>,
    /// Wait for the build to finish, polling status.
    #[arg(long)]
    pub wait: bool,
    /// Max seconds to wait when `--wait` is set.
    #[arg(long, default_value_t = 1800)]
    pub timeout: u64,
    /// Seconds between polls when `--wait` is set.
    #[arg(long, default_value_t = 3)]
    pub poll: u64,
}

// ---- status ---------------------------------------------------------------

#[derive(Debug, Args)]
pub struct StatusArgs {
    pub job: String,
    /// Build number (defaults to the last build).
    pub build_number: Option<u64>,
    /// Resolve status from a queue item URL instead of job+number.
    #[arg(long)]
    pub queue: Option<String>,
}

// ---- job ------------------------------------------------------------------

#[derive(Debug, Args)]
pub struct JobArgs {
    #[command(subcommand)]
    pub cmd: Option<JobCmd>,
}

#[derive(Debug, Subcommand)]
pub enum JobCmd {
    /// List jobs. Default action.
    List,
}

// ---- mcp --------------------------------------------------------------

#[derive(Debug, Args)]
pub struct McpArgs {
    #[command(subcommand)]
    pub cmd: Option<McpCmd>,
}

#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Serve the MCP server over stdio. Default action.
    Serve,
}
