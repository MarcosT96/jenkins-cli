# Contributing

- **Formatting & linting** — Run `cargo fmt` and make sure `cargo clippy` is
  clean (`cargo clippy -- -D warnings`) before opening a pull request.

- **Tests** — Add tests for new behaviour and keep `cargo test` green. HTTP
  client and command-handler tests use [`httpmock`](https://crates.io/crates/httpmock)
  against a local mock server — no real Jenkins instance is needed to run the
  suite.

- **Document any change in behaviour** — Make sure `README.md` and any other
  relevant documentation are kept up-to-date.

- **Consider our release cycle** — We try to follow
  [SemVer v2.0.0](https://semver.org/). Randomly breaking public APIs is not an
  option.

- **One pull request per feature** — If you want to do more than one thing, send
  multiple pull requests.

- **Send coherent history** — Make sure each commit in your pull request is
  meaningful. Squash noisy intermediate commits before submitting.

- **Mutating commands and MCP tools must say so** — Any command or MCP tool
  that triggers a build, changes Jenkins state, or is otherwise irreversible
  should make that clear in its `--help` text or tool description (see
  `trigger_build` in `src/commands/mcp.rs` for the pattern).

## Local setup

```sh
git clone https://github.com/MarcosT96/jenkins-cli
cd jenkins-cli
cargo build
cargo test
```

No live Jenkins server is required to build or test. If you want to exercise
commands against a real instance, `jenkins auth save` (or the `JENKINS_URL` /
`JENKINS_USER` / `JENKINS_TOKEN` env vars) will point the CLI at it.

**Happy coding!**
