use super::EditError;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What an edit replaced: for each file, the content before the first write (`None` when
/// the edit created it) and after the last write.
#[derive(Debug, Default)]
pub struct Snapshot {
    files: BTreeMap<PathBuf, (Option<String>, String)>,
}

impl Snapshot {
    pub(super) fn record(&mut self, path: PathBuf, before: Option<String>, after: String) {
        self.files.insert(path, (before, after));
    }

    pub fn paths(&self) -> Vec<&Path> {
        self.files.keys().map(PathBuf::as_path).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn merge(&mut self, later: Snapshot) {
        for (path, (before, after)) in later.files {
            self.files
                .entry(path)
                .and_modify(|entry| entry.1 = after.clone())
                .or_insert((before, after));
        }
    }

    /// Restores nothing when any file changed since the edit, so another change is never lost.
    pub fn restore(&self) -> Result<(), EditError> {
        for (path, (_, after)) in &self.files {
            if std::fs::read_to_string(path).ok().as_ref() != Some(after) {
                return Err(EditError::ChangedOnDisk(path.clone()));
            }
        }
        for (path, (before, _)) in &self.files {
            match before {
                Some(content) => std::fs::write(path, content)?,
                None => std::fs::remove_file(path)?,
            }
        }
        Ok(())
    }
}
