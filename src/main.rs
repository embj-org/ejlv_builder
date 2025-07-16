use std::process::exit;

mod native;

use ej_builder_sdk::{Action, BuilderEvent, BuilderSdk, prelude::*};

use crate::native::{build_cmake_native, run_native};

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
        Action::Build => build_cmake_native(&sdk).await,
        Action::Run => run_native(&sdk).await,
    }
}
