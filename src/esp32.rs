use std::time::Duration;

use crate::prelude::*;
use ej_builder_sdk::BuilderSdk;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_serial::SerialPortBuilderExt;

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
    let results_p = results_path(&sdk.config_path(), &sdk.board_config_name());
    let _ = std::fs::remove_file(&results_p);

    let board_path = board_folder(&sdk.config_path(), sdk.board_name());

    let result = Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /media/pi/pi_external/esp/esp-idf/export.sh && idf.py -C {} flash",
            board_path.display()
        ))
        .spawn()?
        .wait()
        .await?;

    assert!(result.success());

    // TODO: Create some udev rules to avoid having to hardcode this
    // Fine by now but will need to be done when new boards are added
    let port = tokio_serial::new("/dev/ttyACM0", 115_200)
        .timeout(Duration::from_secs(120))
        .open_native_async()?;

    let mut reader = BufReader::new(port);

    let mut output = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;

        if n == 0 {
            return Err(Error::TimeoutWaitingForBenchmarkToEnd(output));
        }

        output.push_str(&line[..n]);

        if output.contains("Benchmark Over") {
            std::fs::write(results_p, output)?;
            return Ok(());
        }
    }
}
