// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = port_lens_lib::log_capture::run_if_requested() {
        std::process::exit(code);
    }
    port_lens_lib::run()
}
