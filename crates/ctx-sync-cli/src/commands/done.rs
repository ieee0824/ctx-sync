use super::not_implemented;
use crate::cli::DoneArgs;
use ctx_sync_core::Result;

pub fn run(_args: &DoneArgs) -> Result<()> {
    Err(not_implemented("done"))
}
