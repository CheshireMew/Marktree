use std::{
    env,
    io::{self, Read},
    path::PathBuf,
};

use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult, ErrorPayload},
    state::{PersistentState, WorkspaceRuntime},
    types::{SaveDocumentRequest, TextEncoding},
    workspace_operation,
    workspace_service::WorkspaceService,
};

#[derive(Parser)]
#[command(
    name = "marktree-cli",
    version,
    about = "Structured workspace operations for agents and local automation"
)]
struct Cli {
    #[arg(long, global = true, value_name = "DIRECTORY")]
    state_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    pretty: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    Document {
        #[command(subcommand)]
        command: DocumentCommand,
    },
    Folder {
        #[command(subcommand)]
        command: FolderCommand,
    },
    Entry {
        #[command(subcommand)]
        command: EntryCommand,
    },
    Changes {
        #[arg(long)]
        root: String,
    },
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    Inspect {
        #[arg(long)]
        root: String,
    },
    Open {
        #[arg(long)]
        root: String,
    },
}

#[derive(Subcommand)]
enum DocumentCommand {
    List {
        #[arg(long)]
        root: String,
    },
    Read {
        #[arg(long)]
        root: String,
        #[arg(long)]
        path: String,
    },
    Search {
        #[arg(long)]
        root: String,
        #[arg(long)]
        query: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    #[command(group(
        ArgGroup::new("expectation")
            .required(true)
            .multiple(false)
            .args(["expected_sha256", "expected_missing"])
    ))]
    Write {
        #[arg(long)]
        root: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        expected_sha256: Option<String>,
        #[arg(long)]
        expected_missing: bool,
        #[arg(long, value_enum, default_value_t = CliEncoding::Utf8)]
        encoding: CliEncoding,
    },
    WriteBatch,
}

#[derive(Subcommand)]
enum FolderCommand {
    Create {
        #[arg(long)]
        root: String,
        #[arg(long)]
        path: String,
    },
}

#[derive(Subcommand)]
enum EntryCommand {
    Move {
        #[arg(long)]
        root: String,
        #[arg(long)]
        source: String,
        #[arg(long)]
        destination: String,
    },
}

#[derive(Subcommand)]
enum SyncCommand {
    Plan {
        #[arg(long)]
        root: String,
    },
    Run {
        #[arg(long)]
        root: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliEncoding {
    Utf8,
    Utf8Bom,
}

impl From<CliEncoding> for TextEncoding {
    fn from(value: CliEncoding) -> Self {
        match value {
            CliEncoding::Utf8 => Self::Utf8,
            CliEncoding::Utf8Bom => Self::Utf8Bom,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteBatchInput {
    root: String,
    writes: Vec<WriteBatchItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteBatchItem {
    path: String,
    content: String,
    expected_sha256: Option<String>,
    #[serde(default)]
    expected_missing: bool,
    #[serde(default = "default_text_encoding")]
    encoding: TextEncoding,
}

fn default_text_encoding() -> TextEncoding {
    TextEncoding::Utf8
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuccessEnvelope {
    ok: bool,
    command: &'static str,
    data: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    ok: bool,
    command: &'static str,
    error: ErrorPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

pub(crate) fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => {
            let app_error = AppError::Message(error.to_string());
            write_error("arguments", &app_error, false);
            return 2;
        }
        Err(error) => {
            let _ = error.print();
            return 0;
        }
    };
    let pretty = cli.pretty;
    match execute(cli) {
        Ok((command, data)) => {
            if let Some(error) = command_failure(command, &data) {
                let envelope = ErrorEnvelope {
                    ok: false,
                    command,
                    error,
                    data: Some(data),
                };
                let _ = write_json(&envelope, pretty);
                return 1;
            }
            let envelope = SuccessEnvelope {
                ok: true,
                command,
                data,
            };
            if write_json(&envelope, pretty).is_ok() {
                0
            } else {
                1
            }
        }
        Err((command, error)) => {
            write_error(command, &error, pretty);
            1
        }
    }
}

fn execute(cli: Cli) -> Result<(&'static str, serde_json::Value), (&'static str, AppError)> {
    let command_name = command_name(&cli.command);
    let read_only = command_is_read_only(&cli.command);
    let state_dir = cli
        .state_dir
        .map(Ok)
        .unwrap_or_else(default_state_dir)
        .map_err(|error| (command_name, error))?;
    let state = if read_only {
        PersistentState::load_read_only(&state_dir)
    } else {
        PersistentState::load(&state_dir)
    }
    .map_err(|error| (command_name, error))?;
    if !read_only {
        workspace_operation::recover_pending_operations(&state);
    }
    let runtime = WorkspaceRuntime::default();
    let service = if read_only {
        WorkspaceService::new_read_only(&state, &runtime)
    } else {
        WorkspaceService::new(&state, &runtime)
    };

    let result = (|| -> AppResult<serde_json::Value> {
        match cli.command {
            Command::Workspace { command } => match command {
                WorkspaceCommand::Inspect { root } => to_value(service.inspect_workspace(&root)?),
                WorkspaceCommand::Open { root } => to_value(service.open_workspace(&root)?),
            },
            Command::Document { command } => match command {
                DocumentCommand::List { root } => to_value(service.list_workspace_entries(&root)?),
                DocumentCommand::Read { root, path } => {
                    to_value(service.read_document(&root, &path)?)
                }
                DocumentCommand::Search { root, query, limit } => {
                    let client_id = format!("cli:{}", std::process::id());
                    to_value(service.search_documents(&root, &query, limit, &client_id)?)
                }
                DocumentCommand::Write {
                    root,
                    path,
                    expected_sha256,
                    expected_missing,
                    encoding,
                } => {
                    let content = read_stdin()?;
                    to_value(service.save_document(SaveDocumentRequest {
                        root,
                        path,
                        content,
                        expected_sha256,
                        expected_missing,
                        encoding: encoding.into(),
                    })?)
                }
                DocumentCommand::WriteBatch => {
                    let input: WriteBatchInput = serde_json::from_str(&read_stdin()?)?;
                    let requests = batch_requests(input)?;
                    to_value(service.save_documents(requests)?)
                }
            },
            Command::Folder { command } => match command {
                FolderCommand::Create { root, path } => {
                    to_value(service.create_folder(&root, &path)?)
                }
            },
            Command::Entry { command } => match command {
                EntryCommand::Move {
                    root,
                    source,
                    destination,
                } => to_value(service.move_entry(&root, &source, &destination)?),
            },
            Command::Changes { root } => to_value(service.workspace_changes(&root)?),
            Command::Sync { command } => match command {
                SyncCommand::Plan { root } => to_value(service.sync_plan(&root)?),
                SyncCommand::Run { root } => to_value(service.sync(&root)?),
            },
        }
    })();
    result
        .map(|value| (command_name, value))
        .map_err(|error| (command_name, error))
}

fn batch_requests(input: WriteBatchInput) -> AppResult<Vec<SaveDocumentRequest>> {
    input
        .writes
        .into_iter()
        .map(|write| {
            let valid_expectation = write.expected_missing ^ write.expected_sha256.is_some();
            if !valid_expectation {
                return Err(AppError::Message(format!(
                    "Batch write '{}' must provide exactly one of expectedSha256 or expectedMissing.",
                    write.path
                )));
            }
            Ok(SaveDocumentRequest {
                root: input.root.clone(),
                path: write.path,
                content: write.content,
                expected_sha256: write.expected_sha256,
                expected_missing: write.expected_missing,
                encoding: write.encoding,
            })
        })
        .collect()
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::Workspace { command } => match command {
            WorkspaceCommand::Inspect { .. } => "workspace.inspect",
            WorkspaceCommand::Open { .. } => "workspace.open",
        },
        Command::Document { command } => match command {
            DocumentCommand::List { .. } => "document.list",
            DocumentCommand::Read { .. } => "document.read",
            DocumentCommand::Search { .. } => "document.search",
            DocumentCommand::Write { .. } => "document.write",
            DocumentCommand::WriteBatch => "document.writeBatch",
        },
        Command::Folder { .. } => "folder.create",
        Command::Entry { .. } => "entry.move",
        Command::Changes { .. } => "changes",
        Command::Sync { command } => match command {
            SyncCommand::Plan { .. } => "sync.plan",
            SyncCommand::Run { .. } => "sync.run",
        },
    }
}

fn command_is_read_only(command: &Command) -> bool {
    match command {
        Command::Workspace { command } => matches!(command, WorkspaceCommand::Inspect { .. }),
        Command::Document { command } => matches!(
            command,
            DocumentCommand::List { .. }
                | DocumentCommand::Read { .. }
                | DocumentCommand::Search { .. }
        ),
        Command::Changes { .. } => true,
        Command::Sync { command } => matches!(command, SyncCommand::Plan { .. }),
        Command::Folder { .. } | Command::Entry { .. } => false,
    }
}

fn default_state_dir() -> AppResult<PathBuf> {
    if cfg!(target_os = "windows") {
        return env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join("io.github.cheshiremew.marktree"))
            .ok_or_else(|| AppError::Message("APPDATA is not available.".to_owned()));
    }
    if cfg!(target_os = "macos") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| {
                path.join("Library")
                    .join("Application Support")
                    .join("io.github.cheshiremew.marktree")
            })
            .ok_or_else(|| AppError::Message("HOME is not available.".to_owned()));
    }
    env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|path| path.join(".local").join("share"))
        })
        .map(|path| path.join("io.github.cheshiremew.marktree"))
        .ok_or_else(|| AppError::Message("A user data directory is not available.".to_owned()))
}

fn read_stdin() -> AppResult<String> {
    let mut value = String::new();
    io::stdin().read_to_string(&mut value)?;
    Ok(value)
}

fn to_value(value: impl Serialize) -> AppResult<serde_json::Value> {
    Ok(serde_json::to_value(value)?)
}

fn write_error(command: &'static str, error: &AppError, pretty: bool) {
    let envelope = ErrorEnvelope {
        ok: false,
        command,
        error: error.payload(),
        data: None,
    };
    let _ = write_json(&envelope, pretty);
}

fn command_failure(command: &str, data: &serde_json::Value) -> Option<ErrorPayload> {
    if command != "sync.run" || data.get("error").is_none_or(serde_json::Value::is_null) {
        return None;
    }
    serde_json::from_value(data["error"].clone()).ok()
}

fn write_json(value: &impl Serialize, pretty: bool) -> AppResult<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if pretty {
        serde_json::to_writer_pretty(&mut output, value)?;
    } else {
        serde_json::to_writer(&mut output, value)?;
    }
    use std::io::Write;
    output.write_all(b"\n")?;
    Ok(())
}
