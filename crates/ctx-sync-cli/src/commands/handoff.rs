use super::not_implemented;
use crate::cli::HandoffArgs;
use ctx_sync_core::Result;

pub fn run(_args: &HandoffArgs) -> Result<()> {
    Err(not_implemented("handoff"))
}
