//! Authentication commands.
//!
//! `save` prompts for the Jenkins base URL + user + API token and writes them
//! to the config file; `show` prints the saved auth block (token redacted);
//! `whoami` verifies the credentials against the server.

use dialoguer::{Input, Password};

use crate::cli::{AuthArgs, AuthCmd, GlobalArgs};
use crate::client::Client;
use crate::config::{self, Auth, Config};
use crate::error::Result;
use crate::output;

pub fn run(args: AuthArgs, global: &GlobalArgs) -> Result<()> {
    match args.cmd.unwrap_or(AuthCmd::Save) {
        AuthCmd::Save => save(),
        AuthCmd::Show => show(),
        AuthCmd::Whoami => whoami(global),
    }
}

fn save() -> Result<()> {
    output::line(
        "This requires a Jenkins user + API token (Manage Jenkins > Users > \
         your user > Configure > API Token).",
        "yellow",
    );

    let url: String = Input::new()
        .with_prompt("Jenkins URL (e.g. http://10.35.0.51:18080)")
        .interact_text()?;
    let user: String = Input::new().with_prompt("Jenkins user").interact_text()?;
    let api_token: String = Password::new().with_prompt("API token").interact()?;

    let mut config = config::load()?;
    config.auth = Some(Auth {
        url: Some(config::normalize_url(&url)),
        user: Some(user),
        api_token: Some(api_token),
        insecure: false,
    });
    config::save(&config)?;

    output::line("Auth info saved.", "green");
    Ok(())
}

fn show() -> Result<()> {
    let config: Config = config::load()?;
    match config.auth {
        Some(auth) => {
            if let Some(url) = &auth.url {
                output::print_value(&serde_json::json!({ "url": url }));
            }
            if let Some(user) = &auth.user {
                output::print_value(&serde_json::json!({ "user": user }));
            }
            if auth.api_token.is_some() {
                output::print_value(&serde_json::json!({ "apiToken": "********" }));
            }
            if !auth.is_complete() {
                output::line(
                    "Incomplete auth info. Run \"jenkins auth save\" again.",
                    "yellow",
                );
            }
        }
        None => output::line(
            "No auth info saved. Run \"jenkins auth save\" first.",
            "yellow",
        ),
    }
    Ok(())
}

fn whoami(global: &GlobalArgs) -> Result<()> {
    let client = Client::new()?;
    let resp = client.get_raw("/api/json")?;
    let value: serde_json::Value = serde_json::from_str(&resp.body)?;

    if global.json {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        output::line("Credentials are valid.", "green");
    }
    Ok(())
}
