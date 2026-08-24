use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
};

use parking_lot::Mutex;

pub const MAX_SEARCH_RESULTS: usize = 500;

pub struct SearchSession<'a> {
    runtime: &'a WorkspaceRuntime,
    resource_key: String,
    client_id: String,
    generation: u64,
}

impl SearchSession<'_> {
    pub fn is_current(&self) -> bool {
        self.runtime
            .is_search_current(&self.resource_key, &self.client_id, self.generation)
    }
}

impl Drop for SearchSession<'_> {
    fn drop(&mut self) {
        self.runtime
            .finish_search(&self.resource_key, &self.client_id, self.generation);
    }
}

#[derive(Default)]
pub struct WorkspaceRuntime {
    watchers: Mutex<HashMap<(String, String), notify::RecommendedWatcher>>,
    workspace_locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    search_generations: Mutex<HashMap<(String, String), u64>>,
    search_sequence: AtomicU64,
}

impl WorkspaceRuntime {
    pub fn forget_roots(&self, roots: &[String], repository_key: &str) {
        {
            let mut watchers = self.watchers.lock();
            watchers.retain(|(root, _), _| !roots.iter().any(|forgotten| forgotten == root));
        }
        let mut searches = self.search_generations.lock();
        searches.retain(|(resource, _), _| {
            resource != repository_key && !roots.iter().any(|root| root == resource)
        });
        self.workspace_locks.lock().remove(repository_key);
    }

    pub fn has_watcher(&self, root: &str, client_id: &str) -> bool {
        self.watchers
            .lock()
            .contains_key(&(root.to_owned(), client_id.to_owned()))
    }

    pub fn store_watcher(&self, root: &str, client_id: &str, watcher: notify::RecommendedWatcher) {
        self.watchers
            .lock()
            .insert((root.to_owned(), client_id.to_owned()), watcher);
    }

    pub fn remove_watcher(&self, root: &str, client_id: &str) {
        self.watchers
            .lock()
            .remove(&(root.to_owned(), client_id.to_owned()));
    }

    pub fn cancel_search(&self, resource_key: &str, client_id: &str) {
        self.search_generations
            .lock()
            .remove(&(resource_key.to_owned(), client_id.to_owned()));
    }

    pub fn workspace_mutex(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.workspace_locks.lock();
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        locks.insert(key.to_owned(), Arc::downgrade(&lock));
        lock
    }

    pub fn search_session<'a>(&'a self, resource_key: &str, client_id: &str) -> SearchSession<'a> {
        let mut searches = self.search_generations.lock();
        let key = (resource_key.to_owned(), client_id.to_owned());
        let generation = self
            .search_sequence
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        searches.insert(key, generation);
        SearchSession {
            runtime: self,
            resource_key: resource_key.to_owned(),
            client_id: client_id.to_owned(),
            generation,
        }
    }

    fn finish_search(&self, resource_key: &str, client_id: &str, generation: u64) {
        let mut searches = self.search_generations.lock();
        let key = (resource_key.to_owned(), client_id.to_owned());
        if searches
            .get(&key)
            .is_some_and(|current| *current == generation)
        {
            searches.remove(&key);
        }
    }

    fn is_search_current(&self, resource_key: &str, client_id: &str, generation: u64) -> bool {
        self.search_generations
            .lock()
            .get(&(resource_key.to_owned(), client_id.to_owned()))
            .is_some_and(|current| *current == generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_mutex_entries_expire_when_the_last_operation_releases_them() {
        let runtime = WorkspaceRuntime::default();
        let first = runtime.workspace_mutex("workspace");
        let same = runtime.workspace_mutex("workspace");
        assert!(Arc::ptr_eq(&first, &same));
        drop(first);
        drop(same);

        let replacement = runtime.workspace_mutex("workspace");
        assert_eq!(Arc::strong_count(&replacement), 1);
    }

    #[test]
    fn searches_only_cancel_an_older_search_from_the_same_client() {
        let runtime = WorkspaceRuntime::default();
        let first = runtime.search_session("repository", "window-a");
        let other = runtime.search_session("repository", "window-b");
        let second = runtime.search_session("repository", "window-a");

        assert!(!first.is_current());
        assert!(second.is_current());
        assert!(other.is_current());

        drop(first);
        assert!(second.is_current());
        drop(second);

        let third = runtime.search_session("repository", "window-a");
        assert!(third.is_current());
        runtime.cancel_search("repository", "window-a");
        assert!(!third.is_current());
    }
}
