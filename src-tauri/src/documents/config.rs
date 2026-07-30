use std::fs;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{
    error::{AppError, AppResult},
    file_version::{hash_bytes, verify_expected_version},
    git,
    paths::{
        atomic_write, canonical_root, normalize_relative, resolve_existing_file, resolve_for_write,
    },
    state::PersistentState,
    types::{
        SaveWorkspaceConfigRequest, WorkspaceChangeOperation, WorkspaceConfig,
        WorkspaceConfigSnapshot,
    },
};

pub fn read_workspace_config(root: &str) -> AppResult<WorkspaceConfigSnapshot> {
    let root_path = canonical_root(root)?;
    let relative = ".marktree/config.json";
    let path = resolve_for_write(&root_path, relative)?;
    if !path.exists() {
        return Ok(WorkspaceConfigSnapshot {
            config: WorkspaceConfig::default(),
            sha256: None,
            missing: true,
        });
    }
    let path = resolve_existing_file(&root_path, relative)?;
    let bytes = fs::read(path)?;
    let config: WorkspaceConfig = serde_json::from_slice(&bytes)?;
    normalize_relative(&config.assets_dir)?;
    build_ignore_set(&config.ignore_rules)?;
    Ok(WorkspaceConfigSnapshot {
        config,
        sha256: Some(hash_bytes(&bytes)),
        missing: false,
    })
}

pub fn save_workspace_config(
    request: SaveWorkspaceConfigRequest,
    app_state: &PersistentState,
) -> AppResult<WorkspaceConfigSnapshot> {
    let root_path = canonical_root(&request.root)?;
    let assets_dir = normalize_relative(&request.config.assets_dir)?;
    build_ignore_set(&request.config.ignore_rules)?;
    let normalized = WorkspaceConfig {
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
    if git::has_git_capability(&request.root) {
        app_state.record_workspace_change(
            &request.root,
            ".marktree/config.json",
            WorkspaceChangeOperation::Upsert,
            Some(&sha256),
        )?;
    }
    atomic_write(&path, &bytes)?;
    Ok(WorkspaceConfigSnapshot {
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
