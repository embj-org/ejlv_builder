use ej_builder_sdk::BuilderSdk;
use tokio::process::Command;
use tracing::info;

use crate::{board_folder, lvgl_folder, prelude::*, results_path};

pub async fn build_stm32(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();

    let project_path = board_folder(&sdk.config_path(), "lv_port_stm32u5g9j-dk2");
    let gen_lv_conf_script_path = lvgl_folder(&sdk.config_path())
        .join("scripts")
        .join("generate_lv_conf.py");

    let conf_template_path = lvgl_folder(&sdk.config_path()).join("lv_conf_template.h");
    let defaults_conf_path = project_path.join(format!("{}.defaults", sdk.board_config_name()));
    let target_lv_conf_h_path = project_path.join("Core").join("Inc").join("lv_conf.h");

    let result = Command::new("python3")
        .arg(gen_lv_conf_script_path)
        .arg("--template")
        .arg(conf_template_path)
        .arg("--defaults")
        .arg(defaults_conf_path)
        .arg("--config")
        .arg(target_lv_conf_h_path)
        .spawn()?
        .wait()
        .await?;

    assert!(result.success(), "Config generation failed");

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

pub async fn run_stm32(sdk: &BuilderSdk) -> Result<()> {
    info!("Benchmark runs on the stm32 are disabled for now.");
    let results_p = results_path(&sdk.config_path(), "stm32u5g9");
    std::fs::write(&results_p, "Skip")?;
    Ok(())
}
