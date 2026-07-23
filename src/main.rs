//! jenkins-cli — a Jenkins CLI + MCP server.
//!
//! `main` stays thin: parse the CLI, dispatch to a command handler, and catch
//! a single top-level error to print in red.

mod cli;
mod client;
mod commands;
mod config;
mod error;
mod models;
mod output;

use clap::Parser;
use owo_colors::OwoColorize;

use cli::{Cli, Command};
use error::Result;

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("{}", err.to_string().red());
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Auth(args) => commands::auth::run(args, &cli.global),
        Command::Build(args) => commands::build::run(args, &cli.global),
        Command::Status(args) => commands::status::run(args, &cli.global),
        Command::Job(args) => commands::job::run(args, &cli.global),
        Command::Mcp(args) => commands::mcp::run(args, &cli.global),
    }
}
