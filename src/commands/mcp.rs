//! MCP server. Implemented in Phase 4.

use crate::cli::{GlobalArgs, McpArgs};
use crate::error::{AppError, Result};

pub fn run(_args: McpArgs, _global: &GlobalArgs) -> Result<()> {
    Err(AppError::Usage(
        "`jenkins mcp serve` is not implemented yet.".into(),
    ))
}
