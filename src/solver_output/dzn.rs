use super::Status;

pub const SOLUTION_TERMINATOR: &str = "----------";
pub const DONE_TERMINATOR: &str = "==========";
pub const UNSATISFIABLE_TERMINATOR: &str = "=====UNSATISFIABLE=====";
pub const UNBOUNDED_TERMINATOR: &str = "=====UNBOUNDED=====";
pub const UNKNOWN_TERMINATOR: &str = "=====UNKNOWN=====";

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
