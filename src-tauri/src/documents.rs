mod assets;
mod catalog;
mod config;
mod content;
mod operations;

pub(crate) use assets::MAX_ASSET_BYTES;
pub use assets::{read_workspace_preview, write_asset};
#[cfg(test)]
pub use catalog::is_observable_path;
pub use catalog::{
    list_workspace_directories, list_workspace_entries, scan_versioned_workspace_files,
    search_documents, search_documents_filtered, workspace_entries_patch,
};
pub(crate) use catalog::{search_documents_filtered_with_budget, SearchBudget, SearchCriteria};
pub(crate) use config::build_ignore_set;
pub use config::{read_workspace_config, save_workspace_config};
#[cfg(test)]
pub use content::MAX_EDITABLE_DOCUMENT_BYTES;
pub use content::{
    create_document, import_file_from_path, open_document, read_document, read_text_at_root,
    save_document, validate_save_document,
};
pub use operations::{
    create_folder, duplicate_entry, empty_android_trash, list_android_trash, move_entry,
    restore_android_trash, trash_entry,
};

#[cfg(test)]
#[path = "documents/tests.rs"]
mod tests;
