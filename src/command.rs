use anyhow::{format_err, Result};
use std::process::Command;
use std::process::Output;
use std::process::Stdio;

pub fn exec_unhandled(command: &mut Command) -> Result<Output> {
    command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format_err!("Error deploying: {}", e.to_string()))
}

pub fn exec(command: &mut Command) -> Result<Output> {
    let exit = exec_unhandled(command)?;
    if !exit.status.success() {
        std::process::exit(exit.status.code().unwrap_or(1));
    }
    Ok(exit)
}
