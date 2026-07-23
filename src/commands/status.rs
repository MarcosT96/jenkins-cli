//! Build status command. Implemented in Phase 2.

use crate::cli::{GlobalArgs, StatusArgs};
use crate::error::{AppError, Result};

pub fn run(_args: StatusArgs, _global: &GlobalArgs) -> Result<()> {
    Err(AppError::Usage(
        "`jenkins status` is not implemented yet.".into(),
    ))
}
