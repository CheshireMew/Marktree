use std::{env, fs, path::PathBuf};

fn main() {
    let registry_path = PathBuf::from("src/ipc_commands.list");
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let commands = fs::read_to_string(&registry_path)
        .expect("failed to read the IPC command registry")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|command| {
            assert!(
                command
                    .chars()
                    .all(|character| character == '_' || character.is_ascii_alphanumeric()),
                "invalid IPC command name: {command}"
            );
            format!("commands::{command}")
        })
        .collect::<Vec<_>>();
    assert!(!commands.is_empty(), "the IPC command registry is empty");

    let generated = format!(
        "macro_rules! marktree_invoke_handler {{\n    () => {{ tauri::generate_handler![{}] }};\n}}\n",
        commands.join(", ")
    );
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is not set")).join("ipc_handler.rs");
    fs::write(output, generated).expect("failed to generate the Tauri IPC handler");

    tauri_build::build()
}
