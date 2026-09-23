use super::not_implemented;
use crate::cli::ContextArgs;
use ctx_sync_core::Result;

pub fn run(_args: &ContextArgs) -> Result<()> {
    Err(not_implemented("context"))
}
