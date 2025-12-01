use std::{
    path::{Path, PathBuf},
    process::exit,
};

mod error;
mod esp32;
mod native;
mod rzg3e;
mod stm32;
// mod eve;
mod prelude;

use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk};
use tokio::process::Command;
use tracing::info;

use crate::{
    esp32::{build_esp32s3, run_esp32s3},
    native::{build_cmake_native, run_native},
    prelude::*,
    rzg3e::{build_rzg3e, run_rzg3e},
    stm32::{build_stm32, run_stm32},
};

const RZG3E_ADDRESS: &str = "192.168.1.172";

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
    sdk: BuilderSdk,
}
impl BuildProcess {
    pub async fn setup_cmakelists(&self) -> Result<()> {
        info!("Preparing build system for LVGL");
        let original_path = workspace_folder(&self.sdk.config_path()).join("CMakeLists.lvgl.txt");
        let target_path = lvgl_folder(&self.sdk.config_path()).join("CMakeLists.txt");

        let original_folder = workspace_folder(&self.sdk.config_path()).join("cmake-lvgl");
        let target_folder = lvgl_folder(&self.sdk.config_path())
            .join("env_support")
            .join("cmake");
        tokio::fs::copy(original_path, target_path).await?;

        if target_folder.exists() {
            tokio::fs::remove_dir_all(&target_folder).await?;
        }
        tokio::fs::create_dir_all(&target_folder).await?;

        let mut entries = tokio::fs::read_dir(&original_folder).await?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_file() {
                let file_name = entry.file_name();
                let dest_path = target_folder.join(file_name);
                tokio::fs::copy(entry.path(), dest_path).await?;
            }
        }
        Ok(())
    }
}

impl Drop for BuildProcess {
    fn drop(&mut self) {
        info!("Restoring git folder");
        std::process::Command::new("git")
            .arg("-C")
            .arg(lvgl_folder(&self.sdk.config_path()))
            .arg("restore")
            .arg(".")
            .spawn()
            .expect("Failed to spawn git restore process.")
            .wait()
            .expect("Git restore process failed.");
    }
}

pub async fn build(sdk: BuilderSdk) -> Result<()> {
    let build_process = BuildProcess { sdk: sdk.clone() };

    build_process.setup_cmakelists().await?;

    if sdk.board_name() == "SER8" {
        return build_cmake_native(&sdk).await;
    }
    if sdk.board_name() == "esp32s3" {
        return build_esp32s3(&sdk).await;
    }
    if sdk.board_name() == "Renesas RZ/G3E" {
        return build_rzg3e(&sdk).await;
    }
    if sdk.board_name() == "stm32u5g9" {
        return build_stm32(&sdk).await;
    }

    todo!("Implement build for {}", sdk.board_name());
}

pub async fn run(sdk: BuilderSdk) -> Result<()> {
    if sdk.board_name() == "SER8" {
        return run_native(&sdk).await;
    }
    if sdk.board_name() == "esp32s3" {
        return run_esp32s3(&sdk).await;
    }
    if sdk.board_name() == "Renesas RZ/G3E" {
        return run_rzg3e(&sdk).await;
    }
    if sdk.board_name() == "stm32u5g9" {
        return run_stm32(&sdk).await;
    }
    // if sdk.board_name() == "eve" {
    //     return run_eve(&sdk).await;
    // }

    todo!("Implement run for {}", sdk.board_name());
}

async fn kill_application_in_renesas_rzg3e() -> Result<()> {
    let result = Command::new("ssh")
        .arg(format!("root@{RZG3E_ADDRESS}"))
        .arg("killall lvglsim")
        .spawn()?
        .wait()
        .await?;
    assert!(result.success(), "Failed to kill process in Renesas RZ/G3E");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let sdk = BuilderSdk::init(|sdk, event| async move {
        match event {
            BuilderEvent::Exit => {
                if sdk.board_name().starts_with("Renesas") {
                    let _ = kill_application_in_renesas_rzg3e().await;
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
