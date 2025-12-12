use crate::args::DebugVerbosityLevel;
use std::fmt;

#[derive(Debug)]
pub struct Parser {
    input: String,
    objective: Option<f64>,
    debug_verbosity: DebugVerbosityLevel,
}

#[derive(Debug)]
pub enum Output {
    Solution(Solution),
    Status(Status),
}

#[derive(Debug, PartialEq)]
pub enum Status {
    OptimalSolution,
    Unsatisfiable,
    Unbounded,
    Unknown,
}

pub const SOLUTION_TERMINATOR: &str = "----------";
pub const DONE_TERMINATOR: &str = "==========";
pub const UNSATISFIABLE_TERMINATOR: &str = "=====UNSATISFIABLE=====";
pub const UNBOUNDED_TERMINATOR: &str = "=====UNBOUNDED=====";
pub const UNKNOWN_TERMINATOR: &str = "=====UNKNOWN=====";

#[derive(Debug)]
pub struct Solution {
    pub solution: String,
    pub objective: f64,
}

impl Status {
    pub fn to_dzn_string(&self) -> &str {
        match self {
            Status::OptimalSolution => DONE_TERMINATOR,
            Status::Unsatisfiable => UNSATISFIABLE_TERMINATOR,
            Status::Unbounded => UNBOUNDED_TERMINATOR,
            Status::Unknown => UNKNOWN_TERMINATOR,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    JsonParsing(serde_json::Error),
    SolutionMissingObjective,
    Field(String),
    ObjectiveParse,
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::JsonParsing(value)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "failed to parse output")
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl Parser {
    pub fn new(debug_verbosity: DebugVerbosityLevel) -> Self {
        Self {
            input: "".to_owned(),
            objective: None,
            debug_verbosity,
        }
    }

    fn to_solution(&mut self) -> Result<Solution> {
        let objective = self
            .objective
            .take()
            .ok_or(Error::SolutionMissingObjective)?;

        // Clear state
        self.objective = None;
        let input = std::mem::take(&mut self.input);

        Ok(Solution {
            solution: input,
            objective,
        })
    }

    pub fn next_line(&mut self, line: &str) -> Result<Option<Output>> {
        const OBJECTIVE_PREFIX: &str = "_objective = ";

        let line = line.trim();

        self.input += line;
        self.input += "\n";

        if line == SOLUTION_TERMINATOR {
            Ok(Some(Output::Solution(self.to_solution()?)))
        } else if line == DONE_TERMINATOR {
            Ok(Some(Output::Status(Status::OptimalSolution)))
        } else if line == UNSATISFIABLE_TERMINATOR {
            Ok(Some(Output::Status(Status::Unsatisfiable)))
        } else if line == UNBOUNDED_TERMINATOR {
            Ok(Some(Output::Status(Status::Unbounded)))
        } else if line == UNKNOWN_TERMINATOR {
            Ok(Some(Output::Status(Status::Unknown)))
        } else if line.starts_with(OBJECTIVE_PREFIX) {
            let objective_str: String = line[OBJECTIVE_PREFIX.len()..]
                .chars()
                .take_while(|c| *c != ';')
                .collect();
            let objective = objective_str
                .parse::<f64>()
                .map_err(|_| Error::ObjectiveParse)?;
            self.objective = Some(objective);
            Ok(None)
        } else {
            Ok(None)
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     const ARITHMETIC_TARGET_SOLUTION: &str = r#"{"type": "solution", "output": {"default": "yCoor = [29, 1, 8, 6, 31, 15, 11, 6, 6, 1, 42, 11, 40, 26, 37, 16, 16, 43, 21, 33];\nS = [22, 41, 29];\nD = 45;\nobjective = 137;\n", "raw": "yCoor = [29, 1, 8, 6, 31, 15, 11, 6, 6, 1, 42, 11, 40, 26, 37, 16, 16, 43, 21, 33];\nS = [22, 41, 29];\nD = 45;\nobjective = 137;\n", "json": {  "yCoor" : [29, 1, 8, 6, 31, 15, 11, 6, 6, 1, 42, 11, 40, 26, 37, 16, 16, 43, 21, 33],  "objective" : 137,  "S" : [22, 41, 29],  "D" : 45,  "_objective" : 137}}, "sections": ["default", "raw", "json"]}"#;
//     const ARITHMETIC_TARGET_SOLUTION_DZN: &str = "yCoor = [29, 1, 8, 6, 31, 15, 11, 6, 6, 1, 42, 11, 40, 26, 37, 16, 16, 43, 21, 33];\nS = [22, 41, 29];\nD = 45;\nobjective = 137;\n";
//     const ARITHMETIC_TARGET_STATUS: &str = r#"{"type": "status", "status": "UNKNOWN"}"#;
//     const COMMENT: &str = r#"{"type": "comment", "comment": "% obj = 848\n"}"#;
//
//     const NFC_STATUS: &str = r#"{"type": "status", "status": "OPTIMAL_SOLUTION"}"#;
//
//     #[test]
//     fn test_parse_solution() {
//         let input = ARITHMETIC_TARGET_SOLUTION;
//         let output = Output::parse(input, DebugVerbosityLevel::Quiet).unwrap();
//         let Output::Solution(solution) = output else {
//             panic!("Output is not a solution");
//         };
//         assert_eq!(solution.objective, 137.0);
//         assert_eq!(solution.solution, ARITHMETIC_TARGET_SOLUTION_DZN);
//     }
//
//     #[test]
//     fn test_parse_unknown_status() {
//         let input = ARITHMETIC_TARGET_STATUS;
//         let output = Output::parse(input, DebugVerbosityLevel::Quiet).unwrap();
//         let Output::Status(status) = output else {
//             panic!("Output is not a status");
//         };
//         assert_eq!(status, Status::Unknown);
//     }
//
//     #[test]
//     fn test_parse_optimal_status() {
//         let input = NFC_STATUS;
//         let output = Output::parse(input, DebugVerbosityLevel::Quiet).unwrap();
//         let Output::Status(status) = output else {
//             panic!("Output is not a status");
//         };
//         assert_eq!(status, Status::OptimalSolution);
//     }
//
//     #[test]
//     fn test_parse_comment() {
//         let input = COMMENT;
//         let output = Output::parse(input, DebugVerbosityLevel::Quiet).unwrap();
//         let Output::Ignore = output else {
//             panic!("Output is not a status");
//         };
//     }
// }
