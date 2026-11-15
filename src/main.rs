//! The thin binary shell over the library. The shell carries
//! the command line door of PORT11 alone. It hands the process
//! arguments after the program name to the door and exits with
//! the door's code.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    ExitCode::from(ostensible::cli::run(&args) as u8)
}
