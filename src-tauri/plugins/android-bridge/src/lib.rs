use serde::{Deserialize, Serialize};
use tauri::{
    plugin::{PluginHandle, TauriPlugin},
    Manager, Runtime,
};

const PLUGIN_IDENTIFIER: &str = "io.github.cheshiremew.marktree.bridge";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomingShare {
    pub text: Option<String>,
    pub subject: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub media_type: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareFileRequest<'a> {
    path: &'a str,
    media_type: &'a str,
    title: &'a str,
}

pub struct AndroidBridge<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> AndroidBridge<R> {
    pub fn take_pending_share(&self) -> Result<Option<IncomingShare>, String> {
        self.0
            .run_mobile_plugin("takePendingShare", ())
            .map_err(|error| error.to_string())
    }

    pub fn share_file(&self, path: &str, media_type: &str, title: &str) -> Result<(), String> {
        self.0
            .run_mobile_plugin::<serde_json::Value>(
                "shareFile",
                ShareFileRequest {
                    path,
                    media_type,
                    title,
                },
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

pub trait AndroidBridgeExt<R: Runtime> {
    fn android_bridge(&self) -> tauri::State<'_, AndroidBridge<R>>;
}

impl<R: Runtime, T: Manager<R>> AndroidBridgeExt<R> for T {
    fn android_bridge(&self) -> tauri::State<'_, AndroidBridge<R>> {
        self.state::<AndroidBridge<R>>()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("android-bridge")
        .setup(|app, api| {
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "SharePlugin")?;
            app.manage(AndroidBridge(handle));
            Ok(())
        })
        .build()
}
