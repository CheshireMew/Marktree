#![cfg_attr(target_env = "msvc", allow(linker_messages))]

fn main() {
    std::process::exit(marktree_lib::run_cli());
}
