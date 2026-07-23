# jenkins-cli

A Jenkins CLI and MCP server, written in Rust. Trigger builds, check status,
tail console logs, and let an AI assistant (Claude Code, or any MCP client)
run deploys for you.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/MarcosT96/jenkins-cli/main/install.sh | sh
```

Or via Homebrew:

```sh
brew install MarcosT96/tap/jenkins-cli
```

Or build from source:

```sh
git clone https://github.com/MarcosT96/jenkins-cli
cd jenkins-cli
cargo build --release
```

## Auth

```sh
jenkins auth save
```

Prompts for your Jenkins URL, user, and API token — generate one at
`Manage Jenkins > Users > <you> > Configure > API Token`. Authenticates with
HTTP Basic auth (`user:token`), which does not require a CSRF crumb.
Credentials are stored at `~/.jenkins-cli-config.json` (mode `0600`).

Environment variables override the config file, useful for CI or containers:

```sh
export JENKINS_URL=http://10.35.0.51:18080
export JENKINS_USER=mtomassi
export JENKINS_TOKEN=...
export JENKINS_INSECURE=1   # accept self-signed TLS certs, if needed
```

```sh
jenkins auth whoami   # verify credentials
jenkins auth show     # print saved auth (token redacted)
```

## Commands

```sh
# List jobs
jenkins job list

# Check the last build's status
jenkins status code-01-deploy-prod-react-cms-gamepack-all

# Check a specific build number
jenkins status code-01-deploy-prod-react-cms-gamepack-all 19

# Tail a build's console log (defaults to the last build)
jenkins logs code-01-deploy-prod-react-cms-gamepack-all
jenkins logs code-01-deploy-prod-react-cms-gamepack-all 19
jenkins logs code-01-deploy-prod-react-cms-gamepack-all --follow

# Trigger a build and return immediately (prints a queue URL)
jenkins build code-01-deploy-prod-react-cms-gamepack-all

# Trigger a build with parameters
jenkins build my-job --param branch=main --param env=staging

# Trigger a build and wait for it to finish
jenkins build code-01-deploy-prod-react-cms-gamepack-all --wait
jenkins build my-job --wait --timeout 900 --poll 5
```

Add `--json` to any command for raw JSON output instead of colorized
key/value lines — useful for scripting.

`build --wait` exits non-zero if the final result isn't `SUCCESS`, so it
gates cleanly in shell scripts and CI.

## MCP server

```sh
jenkins mcp serve
```

Exposes Jenkins over the Model Context Protocol (stdio transport) so an AI
assistant can trigger and monitor deploys during a conversation. Register it
with Claude Code:

```sh
claude mcp add jenkins-mcp --scope user -- /path/to/jenkins mcp serve
```

Tools exposed:

- **`trigger_build`** *(destructive — starts a real build/deploy)*: trigger a
  job, optionally with `params` and `wait: true` to block until it finishes.
- **`get_build_status`** *(read-only)*: resolve status by job + build number,
  by a queue URL (returned by `trigger_build` when not waiting), or the job's
  last build.
- **`get_console_log`** *(read-only)*: fetch a build's console log. Returns
  only the last 100 lines by default to avoid flooding the assistant's
  context — pass `full: true` for everything. Use this to see why a build
  failed.

## How it works

Jenkins' `POST /job/{job}/build` doesn't return the build directly — it
returns a **queue item** location (in the `Location` response header), since
the build may wait for a free executor. `jenkins build --wait` and
`trigger_build` follow that chain automatically:

1. `POST /job/{job}/build` (or `buildWithParameters`) → `Location` header →
   queue item URL.
2. Poll `{queueUrl}/api/json` until Jenkins assigns an `executable` (or
   reports `cancelled`).
3. Poll `{buildUrl}/api/json` until `building: false`, then read `result`.

## Status

Core functionality (auth, build trigger + wait, status, console logs, MCP
server) is implemented and tested against a real Jenkins instance.
