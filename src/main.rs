use async_recursion::async_recursion;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    pin::Pin,
    process::exit,
};

mod config;
mod error;
mod esp32;
mod native;
mod prelude;
mod rzg3e;
mod stm32;

use config::EjLvBuilderConfig;
use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk};
use tokio::process::Command;
use tracing::{error, info};

use crate::{
    esp32::{build_esp32s3, run_esp32s3},
    native::{build_cmake_native, run_native},
    prelude::*,
    rzg3e::{build_rzg3e, kill_rzg3e, run_rzg3e},
    stm32::{build_stm32, run_stm32},
};

type BuildFn = fn(&BuilderSdk) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
type RunFn = fn(&BuilderSdk) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
type KillFn = fn(&BuilderSdk) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

struct BoardConfig {
    build_fn: BuildFn,
    run_fn: RunFn,
    kill_fn: KillFn,
}

async fn no_kill() -> Result<()> {
    Ok(())
}

fn get_board_configs() -> HashMap<&'static str, BoardConfig> {
    let mut configs = HashMap::new();

    configs.insert(
        "SER8",
        BoardConfig {
            build_fn: |sdk| Box::pin(build_cmake_native(sdk)),
            run_fn: |sdk| Box::pin(run_native(sdk)),
            kill_fn: |_| Box::pin(no_kill()),
        },
    );

    configs.insert(
        "esp32s3",
        BoardConfig {
            build_fn: |sdk| Box::pin(build_esp32s3(sdk)),
            run_fn: |sdk| Box::pin(run_esp32s3(sdk)),
            kill_fn: |_| Box::pin(no_kill()),
        },
    );

    configs.insert(
        "Renesas RZ/G3E",
        BoardConfig {
            build_fn: |sdk| Box::pin(build_rzg3e(sdk)),
            run_fn: |sdk| Box::pin(run_rzg3e(sdk)),
            kill_fn: |sdk| Box::pin(kill_rzg3e(sdk)),
        },
    );

    configs.insert(
        "stm32u5g9",
        BoardConfig {
            build_fn: |sdk| Box::pin(build_stm32(sdk)),
            run_fn: |sdk| Box::pin(run_stm32(sdk)),
            kill_fn: |_| Box::pin(no_kill()),
        },
    );

    configs
}

pub fn workspace_folder(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap().to_path_buf()
}

pub fn lvgl_folder(config_path: &Path) -> PathBuf {
    workspace_folder(config_path).join("lvgl").to_path_buf()
}

pub fn lvgl_cmakelists(config_path: &Path) -> PathBuf {
    lvgl_folder(config_path)
        .join("CMakeLists.txt")
        .to_path_buf()
}

pub fn lvgl_snapshot_cmakelists(config_path: &Path) -> PathBuf {
    workspace_folder(config_path).join("CMakeLists.lvgl.txt")
}

pub fn board_folder(config_path: &Path, board_name: &str) -> PathBuf {
    workspace_folder(config_path).join(board_name)
}

fn results_path(config_path: &Path, config_name: &str) -> PathBuf {
    workspace_folder(config_path).join(format!("results-{}", config_name))
}

struct BuildProcess {
    config_path: PathBuf,
    /// Parsed workspace-level config controlling where build files come from.
    ej_config: EjLvBuilderConfig,
}

impl BuildProcess {
    fn lvgl_repo_path(&self) -> PathBuf {
        workspace_folder(&self.config_path).join("lvgl-master")
    }

    async fn update_lvgl_repo(&self) -> Result<()> {
        let repo_path = self.lvgl_repo_path();
        let remote = &self.ej_config.build_files.remote;
        let commit = &self.ej_config.build_files.commit;

        if repo_path.exists() {
            info!("Updating existing LVGL repository (remote={remote}, commit={commit})");

            // Point origin at the configured remote in case it changed.
            let status = Command::new("git")
                .args([
                    "-C",
                    repo_path.to_str().unwrap(),
                    "remote",
                    "set-url",
                    "origin",
                    remote,
                ])
                .status()
                .await?;

            if !status.success() {
                return Err(Error::GitError("Failed to update remote URL".to_string()));
            }

            // Fetch the specific ref so shallow clones also work.
            let status = Command::new("git")
                .args([
                    "-C",
                    repo_path.to_str().unwrap(),
                    "fetch",
                    "--depth",
                    "1",
                    "origin",
                    commit,
                ])
                .status()
                .await?;

            if !status.success() {
                return Err(Error::GitError(format!(
                    "Failed to fetch '{commit}' from '{remote}'"
                )));
            }

            // Reset working tree to the fetched commit.
            let status = Command::new("git")
                .args([
                    "-C",
                    repo_path.to_str().unwrap(),
                    "reset",
                    "--hard",
                    "FETCH_HEAD",
                ])
                .status()
                .await?;

            if !status.success() {
                return Err(Error::GitError("Failed to reset to FETCH_HEAD".to_string()));
            }
        } else {
            info!("Cloning LVGL repository (remote={remote}, commit={commit})");

            // Clone with depth=1 for speed; we'll check out the right ref next.
            let status = Command::new("git")
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--branch",
                    commit,
                    remote,
                    repo_path.to_str().unwrap(),
                ])
                .status()
                .await?;

            // `--branch` works for branches and tags but not bare SHAs.
            // If that failed, fall back to a clone + fetch + reset approach.
            if !status.success() {
                info!("Branch/tag clone failed, attempting SHA checkout");

                let status = Command::new("git")
                    .args(["clone", "--depth", "1", remote, repo_path.to_str().unwrap()])
                    .status()
                    .await?;

                if !status.success() {
                    return Err(Error::GitError(format!(
                        "Failed to clone LVGL repository from '{remote}'"
                    )));
                }

                let status = Command::new("git")
                    .args([
                        "-C",
                        repo_path.to_str().unwrap(),
                        "fetch",
                        "--depth",
                        "1",
                        "origin",
                        commit,
                    ])
                    .status()
                    .await?;

                if !status.success() {
                    return Err(Error::GitError(format!(
                        "Failed to fetch commit '{commit}' from '{remote}'"
                    )));
                }

                let status = Command::new("git")
                    .args([
                        "-C",
                        repo_path.to_str().unwrap(),
                        "reset",
                        "--hard",
                        "FETCH_HEAD",
                    ])
                    .status()
                    .await?;

                if !status.success() {
                    return Err(Error::GitError(
                        "Failed to reset to fetched commit".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    async fn copy_file(&self, src_relative: &str, dest_relative: &str) -> Result<()> {
        let src = self.lvgl_repo_path().join(src_relative);
        let dest = lvgl_folder(&self.config_path).join(dest_relative);

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::copy(&src, &dest).await?;
        Ok(())
    }

    async fn copy_directory(&self, src_relative: &str, dest_relative: &str) -> Result<()> {
        let src = self.lvgl_repo_path().join(src_relative);
        let dest = lvgl_folder(&self.config_path).join(dest_relative);

        if dest.exists() {
            tokio::fs::remove_dir_all(&dest).await?;
        }

        tokio::fs::create_dir_all(&dest).await?;

        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let src_file = entry.path();
            let dest_file = dest.join(&file_name);

            if src_file.is_file() {
                tokio::fs::copy(&src_file, &dest_file).await?;
            }
        }

        Ok(())
    }
    #[async_recursion]
    async fn copy_directory_recursive(&self, src: &Path, dest: &Path) -> Result<()> {
        if dest.exists() {
            tokio::fs::remove_dir_all(&dest).await?;
        }

        tokio::fs::create_dir_all(&dest).await?;

        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let src_file = entry.path();
            let dest_file = dest.join(&file_name);

            if src_file.is_file() {
                tokio::fs::copy(&src_file, &dest_file).await?;
            } else if src_file.is_dir() {
                self.copy_directory_recursive(&src_file, &dest_file).await?;
            }
        }
        Ok(())
    }

    async fn copy_cmakelists_files(&self) -> Result<()> {
        self.copy_file("CMakeLists.txt", "CMakeLists.txt").await?;
        self.copy_directory("env_support/cmake", "env_support/cmake")
            .await?;
        self.copy_directory(
            "env_support/cmake/dependencies",
            "env_support/cmake/dependencies",
        )
        .await?;
        Ok(())
    }

    async fn copy_makefile(&self) -> Result<()> {
        self.copy_file("lvgl.mk", "lvgl.mk").await?;
        Ok(())
    }

    async fn copy_scripts(&self) -> Result<()> {
        let src = self.lvgl_repo_path().join("scripts");
        let dst = lvgl_folder(&self.config_path).join("scripts");
        self.copy_directory_recursive(&src, &dst).await?;
        Ok(())
    }

    pub async fn fetch_build_files(&self) -> Result<()> {
        info!("LVGL build system ready");
        self.update_lvgl_repo().await?;

        info!("Copying CMakeLists files");
        self.copy_cmakelists_files().await?;

        info!("Copying makefile");
        self.copy_makefile().await?;

        info!("Copying scripts");
        self.copy_scripts().await?;

        Ok(())
    }
}

impl Drop for BuildProcess {
    fn drop(&mut self) {
        info!("Resetting git folder");

        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(lvgl_folder(&self.config_path))
            .arg("reset")
            .arg("--hard")
            .status();

        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(lvgl_folder(&self.config_path))
            .arg("clean")
            .arg("-fdx")
            .status();
    }
}

pub async fn build(sdk: BuilderSdk) -> Result<()> {
    let workspace = workspace_folder(&sdk.config_path());
    let ej_config = EjLvBuilderConfig::load(&workspace).await?;

    info!(
        "Using build files from remote='{}' commit='{}'",
        ej_config.build_files.remote, ej_config.build_files.commit
    );

    let build_process = BuildProcess {
        config_path: sdk.config_path().clone(),
        ej_config,
    };

    build_process.fetch_build_files().await?;

    let configs = get_board_configs();
    let board_config = configs
        .get(sdk.board_name())
        .expect(&format!("Unsupported board: {}", sdk.board_name()));

    (board_config.build_fn)(&sdk).await
}

pub async fn run(sdk: BuilderSdk) -> Result<()> {
    let configs = get_board_configs();
    let board_config = configs
        .get(sdk.board_name())
        .expect(&format!("Unsupported board: {}", sdk.board_name()));

    (board_config.run_fn)(&sdk).await
}

pub async fn kill(sdk: BuilderSdk) -> Result<()> {
    let configs = get_board_configs();
    let board_config = configs
        .get(sdk.board_name())
        .expect(&format!("Unsupported board: {}", sdk.board_name()));

    (board_config.kill_fn)(&sdk).await
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let sdk = BuilderSdk::init(|sdk, event| async move {
        match event {
            BuilderEvent::Exit => {
                if let Err(err) = kill(sdk).await {
                    error!("Failed to kill application {err}");
                }
                exit(1)
            }
        }
    })
    .await
    .expect("Failed to init builder sdk");

    match sdk.action() {
        Action::Build => build(sdk).await,
        Action::Run => run(sdk).await,
    }
}
