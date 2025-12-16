use itertools::Itertools;

use super::{Error, Features, Result};
use crate::{
    args::DebugVerbosityLevel,
    scheduler::{Portfolio, SolverInfo},
};
use std::process::Command;

pub struct Ai {
    pub command_name: String,
    pub verbosity: DebugVerbosityLevel,
}

impl Ai {
    pub fn new(command_name: String, verbosity: DebugVerbosityLevel) -> Self {
        Self {
            command_name,
            verbosity,
        }
    }
}

impl super::Ai for Ai {
    fn schedule(&mut self, features: &Features, cores: usize) -> Result<Portfolio> {
        if self.verbosity >= DebugVerbosityLevel::Info {
            println!("AI info: Using command {}", self.command_name);
        }
        let mut cmd = Command::new(&self.command_name);
        cmd.arg("-p").arg(cores.to_string());
        cmd.arg(features_to_arg(features));

        let output = cmd.output().map_err(|e| {
            Error::Other(format!(
                "Failed to get command output for '{}': {e}",
                self.command_name
            ))
        })?;

        if self.verbosity >= DebugVerbosityLevel::Error {
            print_stderr(output.stderr);
        }

        if !output.status.success() {
            return Err(Error::Other(format!(
                "Command exited with non-zero status code: {}",
                output.status
            )));
        }

        parse_output_as_schedule(output.stdout)
    }
}

fn features_to_arg(features: &Features) -> String {
    features.iter().map(|feat| feat.to_string()).join(",")
}

fn parse_output_as_schedule(output: Vec<u8>) -> Result<Portfolio> {
    let output = String::from_utf8(output)
        .map_err(|_| Error::Other("Failed to convert command's stdout into a string".to_owned()))?;

    output
        .lines()
        .filter(|line| !line.is_empty())
        .map(parse_output_line)
        .collect()
}

fn parse_output_line(line: &str) -> Result<SolverInfo> {
    let (solver, cores_str) = line.split_once(',').ok_or_else(|| {
        Error::Other(format!(
            "Command output line does not contain a ',': '{line}'"
        ))
    })?;
    let cores = cores_str.parse::<usize>().map_err(|_| {
        Error::Other(format!(
            "Command output cores is not an unsigned integer: '{cores_str}' on the following line: {line}"
        ))
    })?;

    Ok(SolverInfo::new(solver.to_owned(), cores))
}

fn print_stderr(stderr: Vec<u8>) {
    if stderr.is_empty() {
        return;
    }

    match String::from_utf8(stderr) {
        Ok(stderr) => stderr
            .lines()
            .for_each(|line| eprintln!("AI error: {line}")),
        Err(_) => eprintln!("AI error: Failed to convert stderr to string"),
    }
}
