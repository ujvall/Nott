#![windows_subsystem = "windows"]

mod clock;
mod config;
mod layout;
mod renderer;
mod window;

fn main() {
    if let Err(e) = window::run() {
        eprintln!("Error running Nott: {e}");
        std::process::exit(1);
    }
}
