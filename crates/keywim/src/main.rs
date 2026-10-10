mod autostart;
mod compositor;

use std::{env, process};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.as_slice() {
        [] => compositor::run_winit(),
        [flag, backend] if flag == "--backend" && backend == "winit" => {
            compositor::run_winit();
        }
        [flag, backend] if flag == "--backend" && backend == "tty" => {
            if let Err(e) = compositor::tty::run_tty() {
                eprintln!("{e}");
                process::exit(1);
            }
        }
        _ => {
            eprintln!("Usage: keywim [--backend winit]");
            process::exit(2);
        }
    }
}
