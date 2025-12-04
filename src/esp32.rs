use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;

use crate::prelude::*;
use ej_builder_sdk::BuilderSdk;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::sleep;
use tokio_serial::SerialPortBuilderExt;
use tracing::{info, warn};

use crate::{board_folder, results_path};

fn idf_version(sdk: &BuilderSdk) -> &'static str {
    if sdk.board_config_name() == "eve" {
        "5.3.1"
    } else {
        "5.2.5"
    }
}
fn project_path(sdk: &BuilderSdk) -> PathBuf {
    match sdk.board_config_name() {
        "eve" => board_folder(&sdk.config_path(), "eve"),
        "nuttx" => board_folder(&sdk.config_path(), "lv_nuttx"),
        _ => board_folder(&sdk.config_path(), sdk.board_name()),
    }
}
async fn flashing_serial_port(sdk: &BuilderSdk) -> Result<&'static str> {
    let board_config_name = sdk.board_config_name();

    let mac = if board_config_name == "eve" {
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
                idf_version, port
            ))
            .output()
            .await?;

        if String::from_utf8_lossy(&result.stdout).contains(mac) {
            return Ok(port);
        }
    }

    Err(Error::DeviceNotFound(format!(
        "ESP32S3 with MAC address \"{}\"",
        mac
    )))
}

async fn application_serial_port(sdk: &BuilderSdk) -> Result<&'static str> {
    let board_config_name = sdk.board_config_name();

    if board_config_name == "nuttx" {
        Ok("/dev/ttyUSB0")
    } else {
        flashing_serial_port(sdk).await
    }
}

async fn run_idf_command(sdk: &BuilderSdk, command: &str) -> Result<ExitStatus> {
    let idf_version = idf_version(sdk);
    let project_path = project_path(sdk);
    Ok(Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /home/lvgl/esp/esp-idf{}/export.sh && idf.py -C {}  {}",
            idf_version,
            project_path.display(),
            command
        ))
        .spawn()?
        .wait()
        .await?)
}

async fn run_esptool_command(sdk: &BuilderSdk, command: &str) -> Result<ExitStatus> {
    let idf_version = idf_version(sdk);
    Ok(Command::new("bash")
        .arg("-c")
        .arg(&format!(
            ". /home/lvgl/esp/esp-idf{}/export.sh && esptool.py {}",
            idf_version, command
        ))
        .spawn()?
        .wait()
        .await?)
}

async fn build_esp32s3_esp_idf(sdk: &BuilderSdk) -> Result<()> {
    let result = run_idf_command(sdk, "--ccache build").await?;

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
        let result = run_idf_command(sdk, "--ccache build").await?;
        assert!(result.success(), "Build Failed");
    }

    Ok(())
}

async fn nuttx_clean(sdk: &BuilderSdk) -> Result<()> {
    let project_path = project_path(sdk);
    let nuttx_path = project_path.join("nuttx");
    let lvgl_path = project_path
        .join("apps")
        .join("graphics")
        .join("lvgl")
        .join("lvgl");

    let _ = Command::new("make")
        .arg("-C")
        .arg(&nuttx_path)
        .arg("distclean")
        .spawn()?
        .wait()
        .await?;

    /* do this defensively in case distclean's rules weren't generated properly*/
    let result = Command::new("bash")
        .arg("-c")
        .arg(&format!(
            "rm -f $(find -H {} -name '*.o')",
            lvgl_path.display()
        ))
        .spawn()?
        .wait()
        .await?;
    assert!(result.success());

    Ok(())
}

async fn build_esp32s3_nuttx(sdk: &BuilderSdk) -> Result<()> {
    let project_path = project_path(sdk);
    let nuttx_path = project_path.join("nuttx");
    let lvgl_apps = project_path.join("apps").join("graphics").join("lvgl");

    let lvgl_path = lvgl_apps.join("lvgl");
    let nuttx_lvgl_kconfig_path = lvgl_apps.join("Kconfig");

    let lvgl_kconfig_path = lvgl_path.join("Kconfig");

    let source_bin_path = nuttx_path.join("nuttx.bin");
    let target_bin_path = project_path.join("nuttx.bin");
    let esp_hal_path =
        std::path::absolute(project_path.join("espressif").join("esp-hal-3rdparty.git"))?;

    info!("Cleaning nuttx build files");
    nuttx_clean(sdk).await?;

    info!("Setting up nuttx config");
    {
        let mut nuttx_lvgl_kconfig = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(nuttx_lvgl_kconfig_path)
            .await?;

        let mut lvgl_kconfig = OpenOptions::new()
            .read(true)
            .open(lvgl_kconfig_path)
            .await?;

        nuttx_lvgl_kconfig
            .write_all(
                br#"#
# For a description of the syntax of this configuration file,
# see the file kconfig-language.txt in the NuttX tools repository.
#

menuconfig GRAPHICS_LVGL
	bool "Light and Versatile Graphic Library (LVGL)"
	default n
	---help---
		Enable support for the LVGL GUI library.

if GRAPHICS_LVGL

"#,
            )
            .await?;

        tokio::io::copy(&mut lvgl_kconfig, &mut nuttx_lvgl_kconfig).await?;

        nuttx_lvgl_kconfig
            .write_all(
                br#"
config LV_OPTLEVEL
	string "Customize compilation optimization level"
	default ""

endif # GRAPHICS_LVGL
"#,
            )
            .await?;
    }

    info!("Building nuttx");
    let result = Command::new("bash")
        .arg("-c")
        .arg(&format!(
            "cd {} \
            && ./tools/configure.sh -l esp32s3-lcd-ev:lvgl \
            && ESP_HAL_3RDPARTY_URL='lvgl@127.0.0.1:{}' make -j$(nproc) nuttx \
            ",
            nuttx_path.display(),
            esp_hal_path.display(),
        ))
        .spawn()?
        .wait()
        .await?;
    assert!(result.success());

    info!("Saving binary file");
    tokio::fs::copy(source_bin_path, target_bin_path).await?;

    /* we need to clean this build so the lvgl dir isn't polluted with object files*/
    info!("Cleaning nuttx build files");
    nuttx_clean(sdk).await?;

    Ok(())
}

pub async fn build_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    if sdk.board_config_name() == "nuttx" {
        build_esp32s3_nuttx(sdk).await
    } else {
        build_esp32s3_esp_idf(sdk).await
    }
}

pub async fn run_esp32s3(sdk: &BuilderSdk) -> Result<()> {
    let board_config_name = sdk.board_config_name();
    let project_path = project_path(sdk);
    let results_p = results_path(&sdk.config_path(), "esp32s3");

    let _ = std::fs::remove_file(&results_p);

    let flashing_port = flashing_serial_port(sdk).await?;

    if board_config_name == "nuttx" {
        let bin_path = project_path.join("nuttx.bin");
        let result = run_esptool_command(
            sdk,
            &format!(
                "-c esp32s3 -p {} -b 921600 write_flash -fs detect -fm dio -ff \"40m\" 0x0000 {}",
                flashing_port,
                bin_path.display()
            ),
        )
        .await?;
        assert!(result.success());
    } else {
        let result = run_idf_command(sdk, &format!("--port {} flash", flashing_port)).await?;

        assert!(result.success());
    }

    let application_port = application_serial_port(sdk).await?;

    let mut port = tokio_serial::new(application_port, 115_200)
        .timeout(Duration::from_secs(120))
        .open_native_async()?;

    if board_config_name == "nuttx" {
        /* just in case the nuttx prompt isn't ready yet */
        sleep(Duration::from_millis(2000)).await;

        port.write_all(b"my_lvgl_app\n").await?;
        port.flush().await?;
    }

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
