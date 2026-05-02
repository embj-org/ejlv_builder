use serde::Deserialize;
use std::path::Path;

use crate::prelude::*;

#[derive(Debug, Deserialize)]
pub struct BuildFilesConfig {
    /// Git remote URL for the LVGL repository
    /// Defaults to the official LVGL repo
    #[serde(default = "default_remote")]
    pub remote: String,

    /// Git branch, tag, or commit SHA to check out
    /// Defaults to "master"
    #[serde(default = "default_commit")]
    pub commit: String,
}

impl Default for BuildFilesConfig {
    fn default() -> Self {
        Self {
            remote: default_remote(),
            commit: default_commit(),
        }
    }
}

fn default_remote() -> String {
    "https://github.com/lvgl/lvgl.git".to_string()
}

fn default_commit() -> String {
    "master".to_string()
}

#[derive(Debug, Deserialize)]
pub struct EjLvBuilderConfig {
    #[serde(default)]
    pub build_files: BuildFilesConfig,
}

impl EjLvBuilderConfig {
    /// Read config from `ejlv_builder_config.toml` in the workspace folder.
    /// If the file doesn't exist, a default config is returned so existing
    /// workspaces that don't have the file keep working as before.
    pub async fn load(workspace: &Path) -> Result<Self> {
        let path = workspace.join("ejlv_builder_config.toml");

        if !path.exists() {
            return Ok(Self {
                build_files: BuildFilesConfig::default(),
            });
        }

        let contents = tokio::fs::read_to_string(&path).await?;
        let config: Self = toml::from_str(&contents).map_err(|e| {
            Error::ConfigError(format!("Failed to parse ejlv_builder_config.toml: {e}"))
        })?;

        Ok(config)
    }
}
