use ctx_sync_core::Result;

use super::not_implemented;
use crate::cli::DecisionCommand;

pub fn run(command: &DecisionCommand) -> Result<()> {
    match command {
        DecisionCommand::Add(_args) => Err(not_implemented("decision add")),
    }
}
