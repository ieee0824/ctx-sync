use super::GistStore;
use crate::store::SyncState;
use crate::{Error, Result};

pub(super) fn sync_state(_store: &GistStore) -> Result<SyncState> {
    Err(Error::General("not implemented: sync_state".into()))
}
