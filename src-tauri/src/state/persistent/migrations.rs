use super::STATE_SCHEMA_VERSION;

pub(super) fn migrate_state_value(value: &mut serde_json::Value) -> Option<()> {
    if value.get("schemaVersion")?.as_u64()? as u32 == 5 {
        migrate_schema_five(value)?;
    }
    if value.get("schemaVersion")?.as_u64()? as u32 == 6 {
        migrate_schema_six(value)?;
    }
    Some(())
}

fn migrate_schema_five(value: &mut serde_json::Value) -> Option<()> {
    let object = value.as_object_mut()?;
    let workspaces = object
        .remove("repositories")
        .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
    object.insert("workspaces".to_owned(), workspaces);
    let changes = object
        .remove("managedChanges")
        .unwrap_or_else(|| serde_json::json!({}));
    object.insert("workspaceChanges".to_owned(), migrate_change_map(changes)?);
    if let Some(operations) = object
        .get_mut("pendingGitOperations")
        .and_then(serde_json::Value::as_object_mut)
    {
        for operation in operations.values_mut() {
            let operation = operation.as_object_mut()?;
            let changes = operation
                .remove("managedChanges")
                .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
            operation.insert(
                "workspaceChanges".to_owned(),
                migrate_change_array(changes)?,
            );
        }
    }
    object.insert("schemaVersion".to_owned(), serde_json::Value::from(6));
    Some(())
}

fn migrate_schema_six(value: &mut serde_json::Value) -> Option<()> {
    let object = value.as_object_mut()?;
    object.insert(
        "pendingWorkspaceOperations".to_owned(),
        serde_json::json!({}),
    );
    object.insert(
        "schemaVersion".to_owned(),
        serde_json::Value::from(STATE_SCHEMA_VERSION),
    );
    Some(())
}

fn migrate_change_map(value: serde_json::Value) -> Option<serde_json::Value> {
    let mut result = serde_json::Map::new();
    for (root, changes) in value.as_object()? {
        let mut migrated = serde_json::Map::new();
        for (path, change) in changes.as_object()? {
            migrated.insert(path.clone(), migrate_change(change.clone())?);
        }
        result.insert(root.clone(), serde_json::Value::Object(migrated));
    }
    Some(serde_json::Value::Object(result))
}

fn migrate_change_array(value: serde_json::Value) -> Option<serde_json::Value> {
    let values = value
        .as_array()?
        .iter()
        .cloned()
        .map(migrate_change)
        .collect::<Option<Vec<_>>>()?;
    Some(serde_json::Value::Array(values))
}

fn migrate_change(mut value: serde_json::Value) -> Option<serde_json::Value> {
    let change = value.as_object_mut()?;
    let version = change.remove("sha256").unwrap_or(serde_json::Value::Null);
    change.remove("kind");
    change.insert(
        "operation".to_owned(),
        serde_json::Value::String("upsert".to_owned()),
    );
    change.insert("version".to_owned(), version);
    Some(value)
}
