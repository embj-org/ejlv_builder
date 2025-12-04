use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use crate::prelude::*;
use ej_builder_sdk::BuilderSdk;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio_serial::SerialPortBuilderExt;
use tracing::warn;

use crate::{board_folder, results_path};

fn idf_version(sdk: &BuilderSdk) -> &'static str {
    if sdk.board_config_name() == "eve" {
        "5.3.1"
    } else {
        "5.2.5"
    }
}
fn project_path(sdk: &BuilderSdk) -> PathBuf {
    if sdk.board_config_name() == "eve" {
        board_folder(&sdk.config_path(), "eve")
    } else {
        board_folder(&sdk.config_path(), sdk.board_name())
    }
}
async fn serial_port(sdk: &BuilderSdk) -> Result<&'static str> {
    let mac = if sdk.board_config_name() == "eve" {
        "34:85:18:6c:f6:dc"
    } else {
        "30:30:f9:5a:88:00"
    };

    let ports = ["/dev/ttyACM0", "/dev/ttyACM1"];

    for port in ports {
        let idf_version = idf_version(sdk);
        let result = Command::new("bash")
            .arg("-c")
            .arg(&format!(
                ". /home/lvgl/esp/esp-idf{}/export.sh && esptool.py --port {} read_mac",
                idf_version,
                port
            ))
            .output()
            .await?;

        if String::from_utf8_lossy(&result.stdout).contains(mac) {
            return Ok(port);
        }
    }

    Err(Error::DeviceNotFound(format!("ESP32S3 with MAC address \"{}\"", mac.to_string())))
}

async fn run_idf_command(sdk: &BuilderSdk, command: &str) -> Result<ExitStatus> {
    let idf_version = idf_version(sdk);
    let project_path = project_path(sdk);
    Ok(Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /home/lvgl/esp/esp-idf{}/export.sh && idf.py -C {} --ccache {}",
            idf_version,
            project_path.display(),
            command
        ))
        .spawn()?
        .wait()
        .await?)
}

pub async fn build_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    let result = run_idf_command(sdk, "build").await?;

    if !result.success() {
        warn!(
            "Build failed for ESP32. This happens when new source files are added. Performing a clean build"
        );
        // https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/tools/idf-py.html#select-the-target-chip-set-target
        // https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/tools/idf-py.html#reconfigure-the-project-reconfigure
        // `set-target` performs a clean build and reconfigures the project which is important in
        // case files were added or removed from the source tree
        let result = run_idf_command(sdk, &format!("set-target {}", sdk.board_name())).await?;
        assert!(result.success(), "Clean Failed");
        let result = run_idf_command(sdk, "build").await?;
        assert!(result.success(), "Build Failed");
    }

    Ok(())
}

pub async fn run_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    let results_p = results_path(&sdk.config_path(), &sdk.board_config_name());
    let _ = std::fs::remove_file(&results_p);

    let port = serial_port(sdk).await?;

    let result = run_idf_command(sdk, &format!("--port {} flash", port)).await?;

    assert!(result.success());

    let port = tokio_serial::new(port, 115_200)
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
