use std::{
    path::{Path, PathBuf},
    process::{Command, exit},
};

use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk, prelude::*};

fn project_folder(config_path: &Path, board_name: &str) -> PathBuf {
    config_path.parent().unwrap().join(board_name)
}

fn lv_conf_path(config_path: &Path, config_name: &str) -> PathBuf {
    config_path
        .parent()
        .unwrap()
        .join("configs")
        .join(format!("lv_{config_name}.h"))
}

fn build_folder(config_path: &Path, config_name: &str) -> PathBuf {
    config_path
        .parent()
        .unwrap()
        .join("build")
        .join(config_name)
}
fn target_path(config_path: &Path, config_name: &str) -> PathBuf {
    build_folder(config_path, config_name).join("target")
}

fn build_cmake_native(sdk: &BuilderSdk) -> Result<()> {
    let nprocs = num_cpus::get();
    let build_path = build_folder(&sdk.config_path(), sdk.board_config_name());
    let project_path = project_folder(&sdk.config_path(), sdk.board_name());
    let conf_path = lv_conf_path(&sdk.config_path(), sdk.board_config_name());
    Command::new("cmake")
        .arg("-B")
        .arg(&build_path)
        .arg("-S")
        .arg(project_path)
        .arg(format!("-DLV_CONF_PATH={}", conf_path.display()))
        .spawn()?
        .wait()?;

    Command::new("cmake")
        .arg("--build")
        .arg(&build_path)
        .arg("-j")
        .arg(nprocs.to_string())
        .spawn()?
        .wait()?;

    Ok(())
}
fn run_native(sdk: &BuilderSdk) -> Result<()> {
    let path = target_path(&PathBuf::from(sdk.config_path()), sdk.board_config_name());
    Command::new(path).spawn()?.wait()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let sdk = BuilderSdk::init(|_sdk, event| async { match event {
        BuilderEvent::Exit => exit(1),
    }})
    .await
    .expect("Failed to init builder sdk");

    match sdk.action() {
        Action::Build => build_cmake_native(&sdk),
        Action::Run => run_native(&sdk),
    }
}
