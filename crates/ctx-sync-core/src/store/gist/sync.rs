use chrono::{DateTime, FixedOffset};

use super::GistStore;
use crate::store::SyncOutcome;
use crate::{Error, Result};

pub(super) fn sync(_store: &GistStore, _now: DateTime<FixedOffset>) -> Result<SyncOutcome> {
    Err(Error::General("not implemented: sync".into()))
}
