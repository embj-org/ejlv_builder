use std::{
    path::{Path, PathBuf},
    process::exit,
};

mod error;
mod esp32;
mod native;
mod prelude;

use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk};

use crate::{
    esp32::{build_esp32s3, run_esp32s3},
    native::{build_cmake_native, run_native},
    prelude::*,
};

pub fn workspace_folder(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap().to_path_buf()
}
pub fn board_folder(config_path: &Path, board_name: &str) -> PathBuf {
    workspace_folder(config_path).join(board_name)
}

fn results_path(config_path: &Path, config_name: &str) -> PathBuf {
    workspace_folder(config_path).join(format!("results-{}", config_name))
}

pub async fn build(sdk: BuilderSdk) -> Result<()> {
    if sdk.board_name() == "rpi4" {
        return build_cmake_native(&sdk).await;
    }
    if sdk.board_name() == "esp32s3" {
        return build_esp32s3(&sdk).await;
    }

    todo!("Implement build for {}", sdk.board_name());
}

pub async fn run(sdk: BuilderSdk) -> Result<()> {
    if sdk.board_name() == "rpi4" {
        return run_native(&sdk).await;
    }
    if sdk.board_name() == "esp32s3" {
        return run_esp32s3(&sdk).await;
    }

    todo!("Implement run for {}", sdk.board_name());
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let sdk = BuilderSdk::init(|_sdk, event| async {
        match event {
            BuilderEvent::Exit => exit(1),
        }
    })
    .await
    .expect("Failed to init builder sdk");

    match sdk.action() {
        Action::Build => build(sdk).await,
        Action::Run => run(sdk).await,
    }
}
