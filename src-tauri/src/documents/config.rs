use std::fs;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{
    error::{AppError, AppResult},
    file_version::{hash_bytes, verify_expected_version},
    paths::{
        atomic_write, canonical_root, normalize_relative, resolve_existing_file, resolve_for_write,
    },
    state::PersistentState,
    types::{
        ManagedChangeKind, RepositoryConfig, RepositoryConfigSnapshot, SaveRepositoryConfigRequest,
    },
};

pub fn read_repository_config(root: &str) -> AppResult<RepositoryConfigSnapshot> {
    let root_path = canonical_root(root)?;
    let relative = ".marktree/config.json";
    let path = resolve_for_write(&root_path, relative)?;
    if !path.exists() {
        return Ok(RepositoryConfigSnapshot {
            config: RepositoryConfig::default(),
            sha256: None,
            missing: true,
        });
    }
    let path = resolve_existing_file(&root_path, relative)?;
    let bytes = fs::read(path)?;
    let config: RepositoryConfig = serde_json::from_slice(&bytes)?;
    normalize_relative(&config.assets_dir)?;
    build_ignore_set(&config.ignore_rules)?;
    Ok(RepositoryConfigSnapshot {
        config,
        sha256: Some(hash_bytes(&bytes)),
        missing: false,
    })
}

pub fn save_repository_config(
    request: SaveRepositoryConfigRequest,
    app_state: &PersistentState,
) -> AppResult<RepositoryConfigSnapshot> {
    let root_path = canonical_root(&request.root)?;
    let assets_dir = normalize_relative(&request.config.assets_dir)?;
    build_ignore_set(&request.config.ignore_rules)?;
    let normalized = RepositoryConfig {
        assets_dir,
        ignore_rules: request
            .config
            .ignore_rules
            .into_iter()
            .map(|rule| rule.trim().replace('\\', "/"))
            .filter(|rule| !rule.is_empty())
            .collect(),
    };
    let path = resolve_for_write(&root_path, ".marktree/config.json")?;
    verify_expected_version(
        &path,
        request.expected_sha256.as_deref(),
        request.expected_missing,
    )?;
    let bytes = serde_json::to_vec_pretty(&normalized)?;
    let sha256 = hash_bytes(&bytes);
    app_state.record_change(
        &request.root,
        ".marktree/config.json",
        &sha256,
        ManagedChangeKind::RepositoryConfig,
    )?;
    atomic_write(&path, &bytes)?;
    Ok(RepositoryConfigSnapshot {
        config: normalized,
        sha256: Some(sha256),
        missing: false,
    })
}

pub(super) fn build_ignore_set(rules: &[String]) -> AppResult<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for rule in rules {
        let normalized = rule.trim().replace('\\', "/");
        if normalized.is_empty() {
            continue;
        }
        builder.add(
            Glob::new(&normalized)
                .map_err(|error| AppError::Message(format!("Invalid ignore rule: {error}")))?,
        );
    }
    builder
        .build()
        .map_err(|error| AppError::Message(format!("Invalid ignore rules: {error}")))
}
