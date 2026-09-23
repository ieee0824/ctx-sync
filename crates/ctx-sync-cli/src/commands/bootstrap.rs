use super::not_implemented;
use crate::cli::BootstrapArgs;
use ctx_sync_core::Result;

pub fn run(_args: &BootstrapArgs) -> Result<()> {
    Err(not_implemented("bootstrap"))
}
