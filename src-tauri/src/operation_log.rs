use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use parking_lot::Mutex;

use crate::{error::AppResult, paths::atomic_write, types::OperationLogEntry};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const RETAINED_ENTRIES_AFTER_COMPACTION: usize = 500;
const MAX_READ_ENTRIES: usize = 200;

pub(crate) struct OperationLog {
    file_path: PathBuf,
    lock_path: PathBuf,
    write_lock: Mutex<()>,
}

impl OperationLog {
    pub(crate) fn new(app_data_dir: &Path) -> Self {
        Self {
            file_path: app_data_dir.join("operation-log.jsonl"),
            lock_path: app_data_dir.join("operation-log.lock"),
            write_lock: Mutex::new(()),
        }
    }

    pub(crate) fn append(&self, entry: &OperationLogEntry) -> AppResult<()> {
        let _guard = self.write_lock.lock();
        let _file_guard = crate::process_lock::exclusive(&self.lock_path)?;
        self.compact_if_needed()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        serde_json::to_writer(&mut file, entry)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    }

    pub(crate) fn read_recent(&self, limit: usize) -> AppResult<Vec<OperationLogEntry>> {
        let _guard = self.write_lock.lock();
        let _file_guard = crate::process_lock::exclusive(&self.lock_path)?;
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.file_path)?;
        let mut entries = parse_entries(&bytes);
        let limit = limit.clamp(1, MAX_READ_ENTRIES);
        if entries.len() > limit {
            entries.drain(..entries.len() - limit);
        }
        entries.reverse();
        Ok(entries)
    }

    fn compact_if_needed(&self) -> AppResult<()> {
        let should_compact = self
            .file_path
            .metadata()
            .is_ok_and(|metadata| metadata.len() >= MAX_LOG_BYTES);
        if !should_compact {
            return Ok(());
        }
        let bytes = fs::read(&self.file_path)?;
        let entries = parse_entries(&bytes);
        let start = entries
            .len()
            .saturating_sub(RETAINED_ENTRIES_AFTER_COMPACTION);
        let mut compacted = Vec::new();
        for entry in &entries[start..] {
            serde_json::to_writer(&mut compacted, entry)?;
            compacted.push(b'\n');
        }
        atomic_write(&self.file_path, &compacted)
    }
}

fn parse_entries(bytes: &[u8]) -> Vec<OperationLogEntry> {
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_slice(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::types::{OperationLogCategory, OperationLogOutcome};

    fn entry(action: &str) -> OperationLogEntry {
        OperationLogEntry {
            timestamp: Utc::now().to_rfc3339(),
            category: OperationLogCategory::Workspace,
            action: action.to_owned(),
            phase: "prepared".to_owned(),
            outcome: OperationLogOutcome::Started,
            root: Some("C:\\notes".to_owned()),
            operation_id: Some(action.to_owned()),
            error_code: None,
        }
    }

    #[test]
    fn recent_entries_are_returned_newest_first_and_invalid_lines_are_ignored() {
        let directory = TempDir::new().unwrap();
        let log = OperationLog::new(directory.path());
        log.append(&entry("first")).unwrap();
        log.append(&entry("second")).unwrap();
        let mut file = OpenOptions::new()
            .append(true)
            .open(directory.path().join("operation-log.jsonl"))
            .unwrap();
        file.write_all(b"not-json\n").unwrap();

        let entries = log.read_recent(1).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "second");
    }
}
