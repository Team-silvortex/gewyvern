use std::env;
use std::process;

use gewyvern::leserpent_macos_install::{InstallOptions, execute};

fn main() {
    match InstallOptions::parse(env::args().skip(1)).and_then(|options| execute(&options)) {
        Ok(report) => println!("{}", report.to_json()),
        Err(error) => {
            eprintln!("Leserpent macOS install failed: {error}");
            process::exit(1);
        }
    }
}
