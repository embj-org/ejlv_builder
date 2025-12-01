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
    config_path: PathBuf,
}

impl BuildProcess {
    const LVGL_REPO: &'static str = "https://raw.githubusercontent.com/lvgl/lvgl/master";

    async fn fetch_file_from_github(url: &str, target: &Path) -> Result<()> {
        let response = reqwest::get(url)
            .await
            .expect("Failed to fetch file from github");
        assert!(
            response.status().is_success(),
            "Failed to fetch {}: {}",
            url,
            response.status()
        );

        let content = response
            .bytes()
            .await
            .expect("Failed to get bytes from file");

        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(target, content).await?;
        Ok(())
    }
    async fn fetch_github_tree(
        owner: &str,
        repo: &str,
        path: &str,
        branch: &str,
    ) -> Result<Vec<String>> {
        let api_url = format!(
            "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
            owner, repo, path, branch
        );

        let client = reqwest::Client::new();
        let response = client
            .get(&api_url)
            .header("User-Agent", "lvgl-builder")
            .send()
            .await
            .expect("Failed to fetch github tree");

        assert!(
            response.status().is_success(),
            "Failed to fetch directory listing: {}",
            response.status()
        );

        let response_text = response.text().await.expect("Failed to get response text");
        let items: Vec<serde_json::Value> =
            serde_json::from_str(&response_text).expect("faield to parse response text");
        let files: Vec<String> = items
            .into_iter()
            .filter(|item| item["type"] == "file")
            .filter_map(|item| item["download_url"].as_str().map(String::from))
            .collect();

        Ok(files)
    }

    async fn fetch_cmakelists_files(&self) -> Result<()> {
        let cmakelists_url = format!("{}/CMakeLists.txt", Self::LVGL_REPO);
        let target_path = lvgl_folder(&self.config_path).join("CMakeLists.txt");

        Self::fetch_file_from_github(&cmakelists_url, &target_path).await?;

        let cmake_files =
            Self::fetch_github_tree("lvgl", "lvgl", "env_support/cmake", "master").await?;
        let target_folder = lvgl_folder(&self.config_path)
            .join("env_support")
            .join("cmake");

        for file_url in cmake_files {
            let file_name = file_url.split('/').last().unwrap();
            let target = target_folder.join(file_name);
            Self::fetch_file_from_github(&file_url, &target).await?;
        }

        Ok(())
    }

    async fn fetch_makefile(&self) -> Result<()> {
        let makefile_url = format!("{}/lvgl.mk", Self::LVGL_REPO);
        let target_path = lvgl_folder(&self.config_path).join("lvgl.mk");
        Self::fetch_file_from_github(&makefile_url, &target_path).await?;
        Ok(())
    }

    async fn fetch_build_scripts(&self) -> Result<()> {
        let script_files = Self::fetch_github_tree("lvgl", "lvgl", "scripts", "master").await?;
        let target_folder = lvgl_folder(&self.config_path).join("scripts");

        for file_url in script_files {
            let file_name = file_url.split('/').last().unwrap();
            let target = target_folder.join(file_name);
            Self::fetch_file_from_github(&file_url, &target).await?;
        }

        Ok(())
    }

    pub async fn fetch_build_files_from_master(&self) -> Result<()> {
        info!("Preparing build system for LVGL");
        info!("Fetching Cmakelists files");
        self.fetch_cmakelists_files().await?;
        info!("Fetching makefile");
        self.fetch_makefile().await?;
        info!("Fetching scripts");
        self.fetch_build_scripts().await?;
        info!("LVGL build system Ready");
        Ok(())
    }
}

impl Drop for BuildProcess {
    fn drop(&mut self) {
        info!("Restoring git folder");
        std::process::Command::new("git")
            .arg("-C")
            .arg(lvgl_folder(&self.config_path))
            .arg("restore")
            .arg(".")
            .spawn()
            .expect("Failed to spawn git restore process.")
            .wait()
            .expect("Git restore process failed.");
    }
}

pub async fn build(sdk: BuilderSdk) -> Result<()> {
    let build_process = BuildProcess {
        config_path: sdk.config_path().clone(),
    };

    build_process.fetch_build_files_from_master().await?;

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
