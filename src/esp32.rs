use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::prelude::*;
use ej_builder_sdk::BuilderSdk;
use ej_io::runner::{RunEvent, Runner};
use tokio::{
    process::Command,
    sync::mpsc::{Receiver, channel},
    time::timeout,
};
use tracing::{info, warn};

use crate::{board_folder, results_path};

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
    let _ = std::fs::remove_file(results_path(&sdk.config_path(), &sdk.board_config_name()));

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
    let (tx, rx) = channel(10);
    let runner = Runner::new(command, args);
    let runner_stop = stop.clone();
    let mut handler = tokio::task::spawn(async move { runner.run(tx, runner_stop).await });
    let result = capture_monitor_output(sdk, rx).await;

    // Always kill the monitor process to not leave zombie process running
    stop.store(true, Ordering::Release);
    let timeout_result = timeout(Duration::from_secs(30), &mut handler).await;
    match timeout_result {
        Ok(result) => {
            info!("IDF process finished {:?}", result);
        }
        Err(_timeout) => {
            warn!(
                "Even after force stopping the idf monitor process, the task handling it didn't complete in time. Aborting. \
                        This may mean idf will be left in zombie state"
            );
            handler.abort();
            let result = handler.await;
            info!("Task result after aborting {:?}", result);
        }
    }
    result
}

async fn capture_monitor_output(sdk: &BuilderSdk, mut rx: Receiver<RunEvent>) -> Result<()> {
    let mut output = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            RunEvent::ProcessCreationFailed(error) => {
                return Err(Error::IDFError(format!(
                    "Couldn't start IDF flash monitor command {}",
                    error
                )));
            }
            RunEvent::ProcessEnd(_) => {
                return Err(Error::IDFError(String::from(
                    "IDF run process quit unexpectedly",
                )));
            }
            RunEvent::ProcessNewOutputLine(line) => {
                info!("{}", line);
                /* Concat because `ProcessNewOutputLine` may not provide a full line and the
                 * sentinel value would be cut in half. Also we can them dump the output as the
                 * result*/
                output.push_str(&line);
                if output.contains("Benchmark Over") {
                    std::fs::write(
                        results_path(&sdk.config_path(), &sdk.board_config_name()),
                        output.clone(),
                    )?;

                    return Ok(());
                }
                continue;
            }
            _ => (),
        }
    }
    Ok(())
}
