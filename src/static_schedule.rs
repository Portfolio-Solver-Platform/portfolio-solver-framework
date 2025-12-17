use std::path::Path;

use crate::{
    args::{Args, DebugVerbosityLevel},
    scheduler::{Portfolio, SolverInfo},
};

pub async fn static_schedule(args: &Args, cores: usize) -> Result<Portfolio> {
    let schedule = match args.static_schedule_path.as_ref() {
        Some(path) => get_schedule_from_file(path).await?,
        None => default_schedule(),
    };

    if args.debug_verbosity >= DebugVerbosityLevel::Warning {
        let schedule_cores = schedule_cores(&schedule);
        if schedule_cores != cores {
            eprintln!(
                "The static schedule cores ({schedule_cores}) does not match the framework's designated cores ({cores})"
            );
        }
    }

    Ok(schedule)
}

fn schedule_cores(schedule: &Portfolio) -> usize {
    schedule.iter().map(|solver_info| solver_info.cores).sum()
}

async fn get_schedule_from_file(path: &Path) -> Result<Portfolio> {
    let contents = tokio::fs::read_to_string(path).await?;
    parse_schedule(&contents).map_err(Into::into)
}

pub fn parse_schedule(s: &str) -> std::result::Result<Portfolio, ParseError> {
    s.lines()
        .filter(|line| !line.is_empty())
        .map(parse_schedule_line)
        .collect()
}

fn parse_schedule_line(line: &str) -> std::result::Result<SolverInfo, ParseError> {
    let (solver, cores_str) =
        line.split_once(',')
            .ok_or_else(|| ParseError::LineDoesNotContainComma {
                line: line.to_owned(),
            })?;

    let cores = cores_str
        .parse::<usize>()
        .map_err(|_| ParseError::CoresNotANumber {
            line: line.to_owned(),
            cores_str: cores_str.to_owned(),
        })?;

    Ok(SolverInfo::new(solver.to_owned(), cores))
}

fn default_schedule() -> Portfolio {
    vec![
        SolverInfo::new("coinbc".to_string(), 1),
        SolverInfo::new("gecode".to_string(), 1),
        // SolverInfo::new("picat".to_string(), 1),
        // SolverInfo::new("cp-sat".to_string(), 1),
        // SolverInfo::new("chuffed".to_string(), 1),
        // SolverInfo::new("yuck".to_string(), 1),
        // SolverInfo::new( "xpress".to_string(), cores / 10),
        // SolverInfo::new( "scip".to_string(), cores / 10),
        // SolverInfo::new( "highs".to_string(), cores / 10),
        // SolverInfo::new( "gurobi".to_string(), cores / 10),
        // SolverInfo::new("coinbc".to_string(), cores / 2),
    ]
}

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub enum Error {
    IoError(tokio::io::Error),
    ParseError(ParseError),
}
#[derive(Debug)]
pub enum ParseError {
    LineDoesNotContainComma { line: String },
    CoresNotANumber { line: String, cores_str: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::LineDoesNotContainComma { line } => {
                write!(f, "Command output line does not contain a ',': '{line}'")
            }
            ParseError::CoresNotANumber { line, cores_str } => write!(
                f,
                "Command output cores is not an unsigned integer: '{cores_str}' on the following line: {line}"
            ),
        }
    }
}

impl From<tokio::io::Error> for Error {
    fn from(value: tokio::io::Error) -> Self {
        Error::IoError(value)
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Error::ParseError(value)
    }
}
