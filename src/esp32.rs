use std::sync::{Arc, atomic::AtomicBool};

use ej_builder_sdk::{BuilderSdk, prelude::*};
use ej_io::runner::{RunEvent, Runner};
use tokio::{process::Command, sync::mpsc::channel};

use crate::board_folder;

pub async fn build_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    let board_path = board_folder(&sdk.config_path(), sdk.board_name());

    let result = Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /media/pi/pi_external/esp/esp-idf/export.sh && idf.py -C {} build",
            board_path.display()
        ))
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());

    Ok(())
}

pub async fn run_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    let board_path = board_folder(&sdk.config_path(), sdk.board_name());

    let command = "bash";
    let args = vec![
        "-c".to_string(),
        format!(
            ". /media/pi/pi_external/esp/esp-idf/export.sh && idf.py -C {} flash monitor",
            board_path.display()
        ),
    ];

    let stop = Arc::new(AtomicBool::new(false));
    let (tx, mut rx) = channel(10);

    let runner = Runner::new(command, args);

    tokio::task::spawn(async move { runner.run(tx, stop).await });

    let mut output = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            RunEvent::ProcessCreationFailed(error) => {
                assert!(false, "Failed to create esp32s3 run process {error}");
            }
            RunEvent::ProcessEnd(_) => {
                assert!(false, "The process should never end");
            }
            RunEvent::ProcessNewOutputLine(line) => {
                println!("{}", line);

                /* Concat because `ProcessNewOutputLine` may not provide a full line and the
                 * sentinel value would be cut in half */
                output.push_str(&line);
                if output.contains("Benchmark Over") {
                    return Ok(());
                }
            }
            _ => (),
        }
    }

    Ok(())
}
