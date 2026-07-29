use std::{collections::HashMap, sync::Arc};

use parking_lot::Mutex;

#[derive(Default)]
pub struct RepositoryRuntime {
    watchers: Mutex<HashMap<String, notify::RecommendedWatcher>>,
    repository_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    search_generations: Mutex<HashMap<String, u64>>,
}

impl RepositoryRuntime {
    pub fn forget_worktrees(&self, worktree_roots: &[String]) {
        {
            let mut watchers = self.watchers.lock();
            for root in worktree_roots {
                watchers.remove(root);
            }
        }
        let mut searches = self.search_generations.lock();
        for root in worktree_roots {
            searches.remove(root);
        }
    }

    pub fn has_watcher(&self, root: &str) -> bool {
        self.watchers.lock().contains_key(root)
    }

    pub fn store_watcher(&self, root: &str, watcher: notify::RecommendedWatcher) {
        self.watchers.lock().insert(root.to_owned(), watcher);
    }

    pub fn repository_mutex(&self, key: &str) -> Arc<Mutex<()>> {
        self.repository_locks
            .lock()
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    pub fn begin_search(&self, key: &str) -> u64 {
        let mut searches = self.search_generations.lock();
        let generation = searches
            .get(key)
            .copied()
            .unwrap_or_default()
            .saturating_add(1);
        searches.insert(key.to_owned(), generation);
        generation
    }

    pub fn is_search_current(&self, key: &str, generation: u64) -> bool {
        self.search_generations
            .lock()
            .get(key)
            .is_some_and(|current| *current == generation)
    }
}
