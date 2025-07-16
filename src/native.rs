use std::path::{Path, PathBuf};

use ej_builder_sdk::{BuilderSdk, prelude::*};
use tokio::process::Command;

fn project_folder(config_path: &Path, board_name: &str) -> PathBuf {
    config_path.parent().unwrap().join(board_name)
}

fn build_folder(config_path: &Path, board_name: &str, config_name: &str) -> PathBuf {
    project_folder(config_path, board_name).join(format!("build-{config_name}"))
}

fn target_path(config_path: &Path, board_name: &str, config_name: &str) -> PathBuf {
    build_folder(config_path, board_name, config_name).join(config_name)
}

pub async fn build_cmake_native(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();

    let build_path = build_folder(
        &sdk.config_path(),
        sdk.board_name(),
        sdk.board_config_name(),
    );

    let project_path = project_folder(&sdk.config_path(), sdk.board_name());
    let conf_path = project_path.join(format!("lv_conf_{}.h", sdk.board_config_name()));

    Command::new("cmake")
        .arg("-B")
        .arg(&build_path)
        .arg("-S")
        .arg(project_path)
        .arg(format!("-DLV_CONF_PATH={}", conf_path.display()))
        .spawn()?
        .wait()
        .await?;

    Command::new("cmake")
        .arg("--build")
        .arg(&build_path)
        .arg("-j")
        .arg(nprocs.to_string())
        .spawn()?
        .wait()
        .await?;

    Ok(())
}
pub async fn run_native(sdk: &BuilderSdk) -> Result<()> {
    let path = target_path(
        &PathBuf::from(sdk.config_path()),
        sdk.board_name(),
        sdk.board_config_name(),
    );

    Command::new(path).spawn()?.wait().await?;
    Ok(())
}
