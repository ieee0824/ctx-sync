use super::not_implemented;
use crate::cli::InitArgs;
use ctx_sync_core::Result;

pub fn run(_args: &InitArgs) -> Result<()> {
    Err(not_implemented("init"))
}
