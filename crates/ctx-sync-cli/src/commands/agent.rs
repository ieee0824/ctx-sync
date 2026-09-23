use ctx_sync_core::Result;

use super::not_implemented;
use crate::cli::AgentCommand;

pub fn run(command: &AgentCommand) -> Result<()> {
    match command {
        AgentCommand::Start(_args) => Err(not_implemented("agent start")),
        AgentCommand::Finish(_args) => Err(not_implemented("agent finish")),
    }
}
