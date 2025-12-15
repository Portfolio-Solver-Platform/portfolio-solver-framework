use crate::args::DebugVerbosityLevel;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

#[derive(Debug)]
pub enum ConversionError {
    CommandFailed(std::process::ExitStatus),
    Other(String),
}

impl From<tokio::io::Error> for ConversionError {
    fn from(value: tokio::io::Error) -> Self {
        Self::Other("Tokio IO error".to_owned())
    }
}

pub struct CachedConverter {
    cache: RwLock<HashMap<String, Conversion>>,
    debug_verbosity: DebugVerbosityLevel,
}

#[derive(Clone)]
pub struct Conversion {
    pub fzn: PathBuf,
    pub ozn: PathBuf,
}

impl CachedConverter {
    pub fn new(debug_verbosity: DebugVerbosityLevel) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            debug_verbosity,
        }
    }

    pub async fn convert(
        &self,
        model: &Path,
        data: Option<&Path>,
        solver_name: &str,
    ) -> Result<Conversion, ConversionError> {
        // TODO: Avoid cloning by making a use_files function that implicitly converts if necessary
        {
            let cache = self.cache.read().await;
            if let Some(conversion) = cache.get(solver_name) {
                return Ok(conversion.clone());
            }
        }

        let conversion = convert_mzn(model, data, solver_name, self.debug_verbosity).await?;
        let mut cache = self.cache.write().await;
        cache.insert(solver_name.to_owned(), conversion.clone());

        Ok(conversion)
    }

    // pub async fn use_files(&self, solver_name: &str, f: impl FnOnce(&Conversion)) {
    //     let conversion = self.cache.read().await.get(solver_name);
    //     if let Some(conversion) = conversion {
    //         f(conversion);
    //     } else {
    //         self.convert()
    //     }
    //     f(path.as_deref().map(|p| p));
    // }
}

pub async fn convert_mzn(
    model: &Path,
    data: Option<&Path>,
    solver_name: &str,
    verbosity: DebugVerbosityLevel,
) -> Result<Conversion, ConversionError> {
    let fzn_file_path = get_new_model_file_name(model, solver_name);
    let ozn_file_path = get_new_ozn_file_name(model, solver_name);
    run_mzn_to_fzn_cmd(
        model,
        data,
        solver_name,
        &fzn_file_path,
        &ozn_file_path,
        verbosity,
    )
    .await?;
    Ok(Conversion {
        fzn: fzn_file_path,
        ozn: ozn_file_path,
    })
}

fn get_new_model_file_name(model: &Path, solver_name: &str) -> PathBuf {
    let new_file_name = format!("_portfolio-model-{solver_name}.fzn");
    model.with_file_name(new_file_name)
}

fn get_new_ozn_file_name(model: &Path, solver_name: &str) -> PathBuf {
    let new_file_name = format!("_portfolio-model-{solver_name}.ozn");
    model.with_file_name(new_file_name)
}

async fn run_mzn_to_fzn_cmd(
    model: &Path,
    data: Option<&Path>,
    solver_name: &str,
    fzn_result_path: &Path,
    ozn_result_path: &Path,
    verbosity: DebugVerbosityLevel,
) -> Result<(), ConversionError> {
    let mut cmd = get_mzn_to_fzn_cmd(model, data, solver_name, fzn_result_path, ozn_result_path);
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    if verbosity >= DebugVerbosityLevel::Warning
        && let Some(stderr) = child.stderr.take()
    {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("MiniZinc compilation: {}", line);
            }
        });
    }

    let status = child.wait().await?;
    if !status.success() {
        return Err(ConversionError::CommandFailed(status));
    }
    Ok(())
}

fn get_mzn_to_fzn_cmd(
    model: &Path,
    data: Option<&Path>,
    solver_name: &str,
    fzn_result_path: &Path,
    ozn_result_path: &Path,
) -> Command {
    let mut cmd = Command::new("minizinc");

    cmd.arg("-c");
    cmd.arg(model);
    if let Some(data) = data {
        cmd.arg(data);
    }
    cmd.args(["--solver", solver_name]);
    cmd.arg("-o").arg(fzn_result_path);
    cmd.args(["--output-objective", "--output-mode", "dzn"]);

    cmd.arg("--ozn");
    cmd.arg(ozn_result_path);

    cmd
}
