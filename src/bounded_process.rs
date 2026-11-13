use std::io;
use std::process::{Command, Output};
use std::time::Duration;

pub(crate) use silvortex_bounded_io::process::OutputLimits;

pub(crate) fn run_command_output(
    command: &mut Command,
    timeout: Duration,
    limits: OutputLimits,
    description: &str,
) -> io::Result<Output> {
    silvortex_bounded_io::process::run_command_output(command, None, timeout, limits, description)
}
