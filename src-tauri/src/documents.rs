mod assets;
mod catalog;
mod config;
mod content;

pub use assets::{read_asset, write_asset};
pub use catalog::{list_documents, search_documents};
pub use config::{read_repository_config, save_repository_config};
pub use content::{
    create_document, open_document, read_document, read_text_at_root, save_document,
};

#[cfg(test)]
#[path = "documents/tests.rs"]
mod tests;
