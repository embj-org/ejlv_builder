use std::path::{Path, PathBuf};

use ej_builder_sdk::BuilderSdk;
use tokio::process::Command;
use tracing::info;

use crate::{board_folder, prelude::*, results_path};

const RZG3E_ADDRESS: &str = "192.168.1.172";

fn build_folder(config_path: &Path, config_name: &str) -> PathBuf {
    board_folder(config_path, "lv_port_linux").join(format!("build-{config_name}"))
}

fn target_path(config_path: &Path, config_name: &str) -> PathBuf {
    build_folder(config_path, config_name)
        .join("bin")
        .join("lvglsim")
}

pub async fn build_rzg3e(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();

    let project_path = board_folder(&sdk.config_path(), "lv_port_linux");

    let build_path = build_folder(&sdk.config_path(), sdk.board_config_name());

    let result = Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /opt/rz-vlp/5.0.8/environment-setup-cortexa55-poky-linux && cmake -DCONFIG={} -B {} -S {}",
            sdk.board_config_name(),
            build_path.display(),
            project_path.display(),
        ))
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

pub async fn run_rzg3e(sdk: &BuilderSdk) -> Result<()> {
    let results_p = results_path(&sdk.config_path(), "renesas-rzg3e");
    let _ = std::fs::remove_file(&results_p);

    let path = target_path(&sdk.config_path(), sdk.board_config_name());

    let result = Command::new("scp")
        .arg(&path)
        .arg(&format!("root@{}:~", RZG3E_ADDRESS))
        .spawn()?
        .wait()
        .await?;

    assert!(result.success(), "SCP execution failed");

    if sdk.board_config_name() == "wayland" {
        Command::new("ssh")
            .arg(&format!("root@{}", RZG3E_ADDRESS))
            .arg(&format!("systemctl start weston"))
            .spawn()?
            .wait_with_output()
            .await?;
    } else {
        Command::new("ssh")
            .arg(&format!("root@{}", RZG3E_ADDRESS))
            .arg(&format!("systemctl stop weston.socket"))
            .spawn()?
            .wait_with_output()
            .await?;
    };

    let result = Command::new("ssh")
        .arg(&format!("root@{}", RZG3E_ADDRESS))
        .arg(&format!("./lvglsim"))
        .spawn()?
        .wait_with_output()
        .await?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);

    // Dump output first so that we have them in the logs before checking if it failed
    info!("{}\n{}", stdout, stderr);

    assert!(result.status.success(), "SSH run failed");

    std::fs::write(&results_p, format!("{}\n{}", stdout, stderr))?;

    Ok(())
}

pub async fn kill_rzg3e(_: &BuilderSdk) -> Result<()> {
    let result = Command::new("ssh")
        .arg(format!("root@{RZG3E_ADDRESS}"))
        .arg("killall lvglsim")
        .spawn()?
        .wait()
        .await?;
    assert!(result.success(), "Failed to kill process in Renesas RZ/G3E");
    Ok(())
}
