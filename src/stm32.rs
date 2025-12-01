use ej_builder_sdk::BuilderSdk;
use tokio::process::Command;
use tracing::info;

use crate::{board_folder, prelude::*};

pub async fn build_stm32(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();

    let project_path = board_folder(&sdk.config_path(), "lv_port_stm32u5g9j-dk2");

    let result = Command::new("make")
        .arg("-C")
        .arg(&project_path)
        .arg("clean")
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());

    let result = Command::new("make")
        .arg("-C")
        .arg(&project_path)
        .arg(format!("-j{}", nprocs))
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());

    Ok(())
}

pub async fn run_stm32(_sdk: &BuilderSdk) -> Result<()> {
    info!("STM32 does not run.");
    Ok(())
}
