use std::path::{Path, PathBuf};

use ej_builder_sdk::BuilderSdk;
use tokio::process::Command;
use tracing::info;

use crate::{board_folder, prelude::*, results_path};

fn build_folder(config_path: &Path, config_name: &str) -> PathBuf {
    board_folder(config_path, "lv_port_linux").join(format!("build-native-{config_name}"))
}

fn target_path(config_path: &Path, config_name: &str) -> PathBuf {
    build_folder(config_path, config_name)
        .join("bin")
        .join("lvglsim")
}

pub async fn build_cmake_native(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();

    let project_path = board_folder(&sdk.config_path(), "lv_port_linux");

    let build_path = build_folder(&sdk.config_path(), sdk.board_config_name());

    let result = Command::new("cmake")
        .arg("-B")
        .arg(&build_path)
        .arg("-S")
        .arg(project_path)
        .arg(format!("-DCONFIG={}", sdk.board_config_name()))
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
    let results_p = results_path(&sdk.config_path(), "ser8");
    if sdk.board_config_name().starts_with("glfw") {
        std::fs::write(&results_p, "Skip")?;
        return Ok(());
    }

    let _ = std::fs::remove_file(&results_p);

    let path = target_path(&sdk.config_path(), sdk.board_config_name());

    let result = Command::new(path).output().await?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Dump output first so that we have them in the logs before checking if it failed
    info!("{}\n{}", stdout, stderr);

    assert!(result.status.success(), "Native run failed");

    std::fs::write(&results_p, format!("{}\n{}", stdout, stderr))?;

    Ok(())
}
