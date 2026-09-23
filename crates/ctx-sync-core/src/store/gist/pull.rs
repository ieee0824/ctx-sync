use super::GistStore;
use crate::store::PullOutcome;
use crate::{Error, Result};

pub(super) fn pull(_store: &GistStore) -> Result<PullOutcome> {
    Err(Error::General("not implemented: pull".into()))
}
