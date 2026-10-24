#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! AKP153E: um executável, dois modos. Serviço (`--background`, dono do USB) e editor (`--editor`).
mod cli;
mod config;
mod editor;
mod engine;
mod graphics;
mod mixer;
mod platform;
mod service;
mod strings;
#[cfg(test)]
mod test_support;
mod usb;

use platform::shell;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if has("--probe") {
        usb::probe();
        return Ok(());
    }
    cli::init_logging();
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
    };
    if has("--autostart") {
        shell::set_autostart(value_of("--autostart").is_some_and(|v| v == "on"));
        return Ok(());
    }
    if has("--run-action") {
        cli::run_action_cli(value_of("--run-action"));
        return Ok(());
    }
    if has("--editor") {
        return cli::run_editor();
    }
    service::run(!has("--background"));
    Ok(())
}
