//! Build trigger command. Implemented in Phase 2.

use crate::cli::{BuildArgs, GlobalArgs};
use crate::error::{AppError, Result};

pub fn run(_args: BuildArgs, _global: &GlobalArgs) -> Result<()> {
    Err(AppError::Usage(
        "`jenkins build` is not implemented yet.".into(),
    ))
}
