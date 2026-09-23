use super::not_implemented;
use crate::cli::AttachArgs;
use ctx_sync_core::Result;

pub fn run(_args: &AttachArgs) -> Result<()> {
    Err(not_implemented("attach"))
}
