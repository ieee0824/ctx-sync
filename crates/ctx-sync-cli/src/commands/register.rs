use super::not_implemented;
use crate::cli::RegisterArgs;
use ctx_sync_core::Result;

pub fn run(_args: &RegisterArgs) -> Result<()> {
    Err(not_implemented("register"))
}
