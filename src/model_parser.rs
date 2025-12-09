use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

#[derive(Debug)]
pub enum ModelParseError {
    NoSolveStatement,
    IoError(std::io::Error),
    RegexError(regex::Error),
}

impl From<std::io::Error> for ModelParseError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

impl From<regex::Error> for ModelParseError {
    fn from(e: regex::Error) -> Self {
        Self::RegexError(e)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectiveType {
    Satisfy,
    Minimize,
    Maximize,
}

impl ObjectiveType {
    pub fn is_better(&self, old: Option<i64>, new: i64) -> bool {
        match (self, old) {
            (_, None) => true,
            (Self::Maximize, Some(val)) => val < new,
            (Self::Minimize, Some(val)) => val > new,
            (Self::Satisfy, _) => true,
        }
    }
}

pub fn insert_objective(
    fzn_path: &PathBuf,
    objective_type: &ObjectiveType,
    objective: i64,
) -> Result<PathBuf, ()> {
    // TODO: Optimise: don't read the entire file, but only read from the end.
    let content = fs::read_to_string(fzn_path).map_err(|_| ())?;
    let content = content.trim();
    let mut lines: Vec<_> = content.lines().collect();

    let solve_line = lines.last().ok_or(())?;
    if !solve_line.starts_with("solve") {
        return Err(());
    }

    let objective_name_rev: String = solve_line
        .chars()
        .rev()
        .skip(1) // Skip the ';'
        .take_while(|c| *c != ' ')
        .collect();
    let objective_name: String = objective_name_rev.chars().rev().collect();
    let objective_constraint =
        get_objective_constraint(objective_type, objective_name.as_str(), objective)?;

    lines.insert(lines.len() - 1, &objective_constraint);

    let new_content = lines.join("\n");
    let file_stem = fzn_path
        .file_stem()
        .unwrap_or_else(|| OsStr::new(""))
        .to_str()
        .ok_or(())?;
    let new_file_path: PathBuf = fzn_path.with_file_name(format!("{file_stem}_{objective}.fzn"));
    fs::write(&new_file_path, new_content).map_err(|_| ())?;

    Ok(new_file_path)
}

fn get_objective_constraint(
    objective_type: &ObjectiveType,
    objective_name: &str,
    objective: i64,
) -> Result<String, ()> {
    fn int_lt(left: &str, right: &str) -> String {
        format!("constraint int_lt({left}, {right});")
    }
    match objective_type {
        ObjectiveType::Satisfy => Err(()),
        ObjectiveType::Minimize => Ok(int_lt(objective_name, &objective.to_string())),
        ObjectiveType::Maximize => Ok(int_lt(&objective.to_string(), objective_name)),
    }
}

pub fn parse_objective_type(model_path: &Path) -> Result<ObjectiveType, ModelParseError> {
    let content = fs::read_to_string(model_path)?;
    let re = Regex::new(r"solve([\S\s]*?;)")?;

    if let Some(cap) = re.captures(&content) {
        let solve_stmt = &cap[1];
        if solve_stmt.contains("minimize") {
            Ok(ObjectiveType::Minimize)
        } else if solve_stmt.contains("maximize") {
            Ok(ObjectiveType::Maximize)
        } else {
            Ok(ObjectiveType::Satisfy)
        }
    } else {
        Err(ModelParseError::NoSolveStatement)
    }
}
