use std::fs;

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::{
    content_policy::VERSIONED_WORKSPACE_CONFIG,
    error::{AppError, AppResult},
    file_version::{hash_bytes, verify_expected_version},
    paths::{
        atomic_write_if_version, canonical_root, normalize_content_relative, resolve_existing_file,
        resolve_for_write,
    },
    state::PersistentState,
    types::{SaveWorkspaceConfigRequest, WorkspaceConfig, WorkspaceConfigSnapshot},
    workspace_operation::{execute_mutation, WorkspaceChangeIntent, WorkspaceOperationKind},
};

pub fn read_workspace_config(root: &str) -> AppResult<WorkspaceConfigSnapshot> {
    let root_path = canonical_root(root)?;
    let relative = VERSIONED_WORKSPACE_CONFIG;
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
    normalize_content_relative(&config.assets_dir)?;
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
    let assets_dir = normalize_content_relative(&request.config.assets_dir)?;
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
    let path = resolve_for_write(&root_path, VERSIONED_WORKSPACE_CONFIG)?;
    verify_expected_version(
        &path,
        request.expected_sha256.as_deref(),
        request.expected_missing,
    )?;
    let bytes = serde_json::to_vec_pretty(&normalized)?;
    let sha256 = hash_bytes(&bytes);
    execute_mutation(
        &request.root,
        WorkspaceOperationKind::WriteFile {
            path: VERSIONED_WORKSPACE_CONFIG.to_owned(),
            version: sha256.clone(),
            previous_version: request.expected_sha256.clone(),
            replace_existing: false,
        },
        vec![WorkspaceChangeIntent::upsert(
            VERSIONED_WORKSPACE_CONFIG,
            &sha256,
        )],
        app_state,
        (),
        |operation| {
            atomic_write_if_version(
                &path,
                &bytes,
                request.expected_sha256.as_deref(),
                request.expected_missing,
                &operation.id,
            )
        },
    )?;
    Ok(WorkspaceConfigSnapshot {
        config: normalized,
        sha256: Some(sha256),
        missing: false,
    })
}

pub(crate) fn build_ignore_set(rules: &[String]) -> AppResult<GlobSet> {
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
