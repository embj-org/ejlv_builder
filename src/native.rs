use std::path::{Path, PathBuf};

use ej_builder_sdk::BuilderSdk;
use tokio::process::Command;

use crate::{board_folder, prelude::*, results_path};

fn build_folder(config_path: &Path, board_name: &str, config_name: &str) -> PathBuf {
    board_folder(config_path, board_name).join(format!("build-{config_name}"))
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

    let project_path = board_folder(&sdk.config_path(), sdk.board_name());
    let conf_path = project_path.join(format!("lv_conf_{}.h", sdk.board_config_name()));

    let result = Command::new("cmake")
        .arg("-B")
        .arg(&build_path)
        .arg("-S")
        .arg(project_path)
        .arg(format!("-DLV_BUILD_CONF_PATH={}", conf_path.display()))
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());

    let result = Command::new("cmake")
        .arg("--build")
        .arg(&build_path)
        .arg("-j")
        .arg(nprocs.to_string())
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());
    Ok(())
}
pub async fn run_native(sdk: &BuilderSdk) -> Result<()> {
    let path = target_path(
        &PathBuf::from(sdk.config_path()),
        sdk.board_name(),
        sdk.board_config_name(),
    );

    let result = Command::new(path).output().await?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Dump output first so that we have them in the logs before checking if it failed
    println!("{}\n{}", stdout, stderr);

    assert!(result.status.success(), "Native run failed");

    std::fs::write(
        results_path(&sdk.config_path(), &sdk.board_config_name()),
        format!("{}\n{}", stdout, stderr),
    )?;

    Ok(())
}
