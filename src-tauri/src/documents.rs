mod assets;
mod catalog;
mod config;
mod content;
mod operations;

pub use assets::{read_asset, write_asset};
pub use catalog::{list_workspace_entries, scan_workspace_files, search_documents};
pub use config::{read_workspace_config, save_workspace_config};
pub use content::{
    create_document, open_document, read_document, read_text_at_root, save_document,
};
pub use operations::{
    create_folder, empty_android_trash, list_android_trash, move_entry, restore_android_trash,
    trash_entry,
};

#[cfg(test)]
#[path = "documents/tests.rs"]
mod tests;
