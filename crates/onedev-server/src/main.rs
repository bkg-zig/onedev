//! OneDev server binary.
//!
//! ```text
//! onedev-server                 run the server
//! onedev-server --check-config  validate the configuration and exit
//! onedev-server --version       print the version and exit
//! ```
//!
//! Configuration: `ONEDEV_CONFIG_DIR` (default `/etc/onedev`) and `ONEDEV_*`
//! environment variables, see [`onedev_server::config`].

use std::process::ExitCode;

use onedev_server::{AppConfig, telemetry};

fn main() -> ExitCode {
    let mut check_only = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--version" | "-V" => {
                println!("onedev-server {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--check-config" => check_only = true,
            "--help" | "-h" => {
                println!(
                    "usage: onedev-server [--check-config | --version]\n\n\
                     Configuration is read from $ONEDEV_CONFIG_DIR (default /etc/onedev)\n\
                     and from ONEDEV_* environment variables."
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("onedev-server: unknown argument `{other}` (try --help)");
                return ExitCode::from(2);
            }
        }
    }

    let config = match AppConfig::load().and_then(|config| config.validate().map(|()| config)) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("onedev-server: {error}");
            return ExitCode::from(2);
        }
    };
    if check_only {
        println!("configuration is valid");
        return ExitCode::SUCCESS;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("onedev-worker")
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("onedev-server: cannot start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(serve(config))
}

async fn serve(config: AppConfig) -> ExitCode {
    // Telemetry spawns its exporter task, so it starts inside the runtime.
    let telemetry = match telemetry::init(&config) {
        Ok(guard) => guard,
        Err(error) => {
            eprintln!("onedev-server: cannot initialise logging: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = onedev_server::run(config).await;
    let code = match &outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("server failed: {error:#}");
            // The subscriber may drop events right after an early failure,
            // so the reason is also written to stderr.
            eprintln!("onedev-server: {error:#}");
            ExitCode::FAILURE
        }
    };
    telemetry.shutdown().await;
    code
}
