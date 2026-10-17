use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherSettings {
    pub heap_mb: u32,
    pub extra_jvm_args: String,
    pub minimize_on_launch: bool,
    pub loader_version: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        LauncherSettings {
            heap_mb: 3072,
            extra_jvm_args: String::new(),
            minimize_on_launch: false,
            loader_version: "0.19.5".into(),
        }
    }
}
