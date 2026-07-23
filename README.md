# jenkins-cli

A Jenkins CLI and MCP server, written in Rust.

## Auth

```
jenkins auth save
```

Prompts for your Jenkins URL, user, and API token (Basic auth, no CSRF crumb
required). Credentials are stored at `~/.jenkins-cli-config.json` (`0600`).

Environment variables override the config file: `JENKINS_URL`, `JENKINS_USER`,
`JENKINS_TOKEN`, `JENKINS_INSECURE`.

## Status

Early scaffolding — `auth save/show/whoami` are implemented. `build`,
`status`, `job list`, and `mcp serve` are coming next.
