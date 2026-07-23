//! Job listing command. Implemented in Phase 3.

use crate::cli::{GlobalArgs, JobArgs};
use crate::error::{AppError, Result};

pub fn run(_args: JobArgs, _global: &GlobalArgs) -> Result<()> {
    Err(AppError::Usage(
        "`jenkins job` is not implemented yet.".into(),
    ))
}
