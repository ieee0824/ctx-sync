//! `index.json`: which project each attached Gist belongs to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StateRoot;
use crate::Result;
use crate::fs_util::{read_json_opt, write_json};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    /// gist id -> project id
    pub projects: BTreeMap<String, Uuid>,
}

impl Index {
    /// Reads `root.index_path()`. Missing file means an empty index.
    pub fn load(root: &StateRoot) -> Result<Self> {
        Ok(read_json_opt(&root.index_path())?.unwrap_or_default())
    }

    pub fn save(&self, root: &StateRoot) -> Result<()> {
        write_json(&root.index_path(), self)
    }

    pub fn get(&self, gist_id: &str) -> Option<Uuid> {
        self.projects.get(gist_id).copied()
    }

    pub fn insert(&mut self, gist_id: impl Into<String>, project_id: Uuid) {
        self.projects.insert(gist_id.into(), project_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_index_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::load(&StateRoot::new(dir.path())).unwrap();
        assert_eq!(index, Index::default());
        assert!(index.get("abc").is_none());
    }

    #[test]
    fn insert_save_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let root = StateRoot::new(dir.path());
        let id = Uuid::new_v4();
        let mut index = Index::default();
        index.insert("abc", id);
        index.save(&root).unwrap();
        let loaded = Index::load(&root).unwrap();
        assert_eq!(loaded, index);
        assert_eq!(loaded.get("abc"), Some(id));
    }
}
