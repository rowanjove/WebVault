// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime for CLI");
        match rt.block_on(webvault_lib::cli::handle_cli(args)) {
            Ok(handled) => {
                if handled {
                    return;
                }
            }
            Err(e) => {
                eprintln!("WebVault CLI Error: {:#}", e);
                std::process::exit(1);
            }
        }
    }

    webvault_lib::run();
}
