use std::{
    path::{Path, PathBuf},
    process::ExitStatus,
};

use serde_json::{Value, json};
use tokio::process::Command;

pub struct Solver {
    name: String,
    supported_std_flags: SupportedStdFlags,
    kind: SolverKind,
}

pub struct SupportedStdFlags {
    a: bool,
    i: bool,
    f: bool,
    p: bool,
}

pub enum SolverKind {
    Fzn,
    Json(Executable),
}

pub struct Executable(PathBuf);

pub type Solvers = Vec<Solver>;

pub async fn discover(minizinc_exe: &Path) -> Result<Solvers> {
    let output = run_discover_command(minizinc_exe).await?;
    parse_discover_output(output)
}

fn parse_discover_output(output: Vec<u8>) -> Result<Solvers> {
    let json = serde_json::from_slice::<Value>(&output)?;
    let Value::Array(array) = json else {
        return Err(Error::InvalidOutputFormat("JSON does not start with an array".to_owned()));
    }
    todo!()
}

async fn run_discover_command(minizinc_exe: &Path) -> Result<Vec<u8>> {
    let mut cmd = Command::new(minizinc_exe);
    cmd.arg("--solvers-json");
    let output = cmd.output().await.map_err(Error::CommandFailed)?;
    if !output.status.success() {
        Err(Error::CommandUnsuccessful(output.status))
    } else {
        Ok(output.stdout)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Discover command failed: {0}")]
    CommandFailed(std::io::Error),
    #[error("Discover command exited with unsuccessful exit code: {0}")]
    CommandUnsuccessful(ExitStatus),
    #[error("Error occurred while parsing solver discovery output as JSON: {0}")]
    JsonParsing(#[from] serde_json::Error),
    #[error("The solver discovery output did not have the expected format: {0}")]
    InvalidOutputFormat(String)
}

pub type Result<T> = std::result::Result<T, Error>;
