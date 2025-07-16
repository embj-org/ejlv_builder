use std::process::exit;

mod native;

use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk, prelude::*};

use crate::native::{build_cmake_native, run_native};

pub fn workspace_folder(config_path: &Path) -> PathBuf {
    config_path.parent().unwrap().to_path_buf()
}
pub fn board_folder(config_path: &Path, board_name: &str) -> PathBuf {
    workspace_folder(config_path).join(board_name)
}

pub async fn build(sdk: BuilderSdk) -> Result<()> {
    if sdk.board_name() == "rpi4" {
        return build_cmake_native(&sdk).await;
    }

    todo!("Implement build for {}", sdk.board_name());
}

pub async fn run(sdk: BuilderSdk) -> Result<()> {
    if sdk.board_name() == "rpi4" {
        return run_native(&sdk).await;
    }

    todo!("Implement run for {}", sdk.board_name());
}

#[tokio::main]
async fn main() -> Result<()> {
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
