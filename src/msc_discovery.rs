use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use regex::Regex;
use crate::logging;

#[derive(Debug, thiserror::Error)]
pub enum MscDiscoveryError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Command failed with status: {0}")]
    CommandFailed(std::process::ExitStatus),
    #[error("Failed to parse minizinc --solvers output")]
    ParseError(String),
    #[error("Failed to parse JSON: {0}")]
    JsonParse(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, MscDiscoveryError>;

#[derive(Debug, Clone)]
pub struct SolverMetadata {
    pub input_type: String,
    pub executable: Option<PathBuf>,
}

pub type SolverMetadataMap = HashMap<String, SolverMetadata>;

/// Discovers all .msc files by running `minizinc --solvers` and searching
/// the solver configuration search paths.
pub async fn discover_msc_files(minizinc_exe: &Path) -> Result<Vec<PathBuf>> {
    let output = run_minizinc_solvers_command(minizinc_exe).await?;
    let search_paths = parse_search_paths(&output)?;
    
    let mut msc_files = Vec::new();
    for path in search_paths {
        match find_msc_files_in_directory(&path).await {
            Ok(mut files) => {
                msc_files.append(&mut files);
            }
            Err(e) => {
                logging::warning!("Failed to search for .msc files in {}: {}", path.display(), e);
            }
        }
    }
    
    Ok(msc_files)
}

/// Discovers all .msc files and parses them to extract solver metadata.
/// Returns a map from solver name/id to metadata (inputType, executable).
/// The map includes entries for both solver IDs (from .msc files) and solver names (from minizinc --solvers).
pub async fn discover_solver_metadata(minizinc_exe: &Path) -> Result<SolverMetadataMap> {
    // First, get the minizinc --solvers output to map solver names to IDs
    let solvers_output = run_minizinc_solvers_command(minizinc_exe).await?;
    let name_to_id_map = parse_solver_names_to_ids(&solvers_output)?;
    
    logging::info!("Parsed {} solver name mappings from minizinc --solvers", name_to_id_map.len());
    
    // Then, discover and parse all .msc files
    let msc_files = discover_msc_files(minizinc_exe).await?;
    let mut metadata_map = HashMap::new();
    
    for msc_file in &msc_files {
        match parse_msc_file(msc_file).await {
            Ok((name, identifier, metadata)) => {
                logging::info!("Parsed .msc file {} with name: {:?} and identifier {:?}", msc_file.display(), name, identifier);
                
                metadata_map.insert(identifier.clone(), metadata.clone());
                metadata_map.insert(name.clone(), metadata.clone());
            }
            Err(e) => {
                logging::warning!("Failed to parse .msc file {}: {}", msc_file.display(), e);
            }
        }
    }
    
    logging::info!("Total solver metadata entries: {}", metadata_map.len());
    
    Ok(metadata_map)
}

fn get_absolute_path(file_path: &Path, exec_path_str: &str) -> PathBuf {
    let exec_path = Path::new(exec_path_str);
    
    // If exec path is absolute, return it directly
    if exec_path.is_absolute() {
        return exec_path.to_path_buf();
    }
    
    // If relative, resolve it relative to the directory containing the file
    let file_dir = Path::new(file_path)
        .parent()
        .ok_or_else(|| std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "File path has no parent directory"
        )).unwrap();
    
    let mut abs_path = file_dir.to_path_buf();
    abs_path.push(exec_path);
    
    // Canonicalize to resolve .. and . components
    return abs_path.canonicalize().unwrap();
}

/// Parses the minizinc --solvers output to extract a mapping from solver names to their IDs.
/// Format: "Solver Name version (id, tag1, tag2, alias1, alias2, ...)"
/// Returns a map from solver name/alias to ID.
fn parse_solver_names_to_ids(output: &str) -> Result<HashMap<String, String>> {
    let mut name_to_id = HashMap::new();
    let lines: Vec<&str> = output.lines().collect();
    
    // Skip until we find "Available solver configurations:"
    let mut in_solvers_section = false;
    
    for line in lines {
        let trimmed = line.trim();
        
        if trimmed == "Available solver configurations:" {
            in_solvers_section = true;
            continue;
        }
        
        if in_solvers_section {
            // Stop when we hit "Search path for solver configurations:"
            if trimmed == "Search path for solver configurations:" {
                break;
            }
            
            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }
            
            // Parse line like: "COIN-BC 2.10.12/1.17.10 (org.minizinc.mip.coin-bc, mip, float, api, osicbc, coinbc, cbc)"
            // or: "choco 5.0.0-beta.1 (org.choco.choco, cp, int)"
            // or: "Chuffed 0.13.2 (org.chuffed.chuffed, cp, lcg, int)"
            if let Some(open_paren) = trimmed.find('(') {
                let before_paren = trimmed[..open_paren].trim();
                // Extract the display name (first word before version)
                let display_name = before_paren.split_whitespace().next().unwrap_or("").to_lowercase();
                
                let after_paren = &trimmed[open_paren + 1..];
                if let Some(close_paren) = after_paren.find(')') {
                    let content = &after_paren[..close_paren];
                    let parts: Vec<&str> = content.split(',').map(|s| s.trim()).collect();
                    
                    if let Some(id) = parts.first() {
                        let id = id.to_string();
                        
                        // Map the ID to itself
                        name_to_id.insert(id.clone(), id.clone());
                        
                        // Map the display name to the ID (both original and lowercase)
                        if !display_name.is_empty() {
                            name_to_id.insert(display_name.clone(), id.clone());
                            // Also try the original case
                            if let Some(orig_name) = before_paren.split_whitespace().next() {
                                name_to_id.insert(orig_name.to_lowercase(), id.clone());
                            }
                        }
                        
                        // Map all other parts (aliases and tags) to the ID
                        // We map everything to be safe, even if some are tags rather than aliases
                        for part in parts.iter().skip(1) {
                            if !part.is_empty() {
                                let part_lower = part.to_lowercase();
                                name_to_id.insert(part_lower, id.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(name_to_id)
}

async fn parse_msc_file(msc_path: &Path) -> Result<(String, String, SolverMetadata)> {
    logging::info!("parsing .msc file {}", msc_path.display());
    let content = tokio::fs::read_to_string(msc_path).await?;
    let executable_regex = Regex::new(r#""executable".*:.*"(.+)",?\n"#).unwrap();
    
    let executable: Option<PathBuf> = executable_regex
        .captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| get_absolute_path(msc_path, m.as_str())); 

    logging::info!("parsing .msc file {}, executable: {:?}", msc_path.display(), executable);
    let input_type_regex = Regex::new(r#""inputType".*:.*"(.+)",?\n"#).unwrap();
    
    let input_type: String = input_type_regex
        .captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .map(String::from)
        .unwrap_or_else(|| "FZN".to_string());
    
    logging::info!("parsing .msc file {}, input_type: {}", msc_path.display(), input_type);

    let metadata = SolverMetadata {
        input_type,
        executable,
    };
    
    let name_regex = Regex::new(r#""name".*:.*"(.+)",?\n"#).unwrap();
    let name: Option<String> = name_regex
        .captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .map(String::from); 

    logging::info!("parsing .msc file {}, name: {:?}", msc_path.display(), name);
    
    if name.is_none(){
        let msg = format!("cannot find name for solver {}", msc_path.display());
        return Err(MscDiscoveryError::ParseError(msg));
    }

    let id_regex = Regex::new(r#""id".*:.*"(.+)",?\n"#).unwrap();
    let id: Option<String> = id_regex
        .captures(&content)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .map(String::from); 

    logging::info!("parsing .msc file {}, id: {:?}", msc_path.display(), id);
    if id.is_none(){
        let msg = format!("cannot find id for solver {}", msc_path.display());
        return Err(MscDiscoveryError::ParseError(msg));
    }

    Ok((name.unwrap(), id.unwrap().rsplit('.').next().unwrap().to_string(), metadata))
}

async fn run_minizinc_solvers_command(minizinc_exe: &Path) -> Result<String> {
    let mut cmd = Command::new(minizinc_exe);
    cmd.arg("--solvers");
    cmd.kill_on_drop(true);
    
    let output = cmd.output().await?;
    
    if !output.status.success() {
        return Err(MscDiscoveryError::CommandFailed(output.status));
    }
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_search_paths(output: &str) -> Result<Vec<PathBuf>> {
    let lines: Vec<&str> = output.lines().collect();
    
    // Find the line "Search path for solver configurations:"
    let mut found_header = false;
    let mut search_paths = Vec::new();
    
    for line in lines {
        let trimmed = line.trim();
        
        if trimmed == "Search path for solver configurations:" {
            found_header = true;
            continue;
        }
        
        if found_header {
            // Empty line or non-path line indicates we're done
            if trimmed.is_empty() {
                break;
            }
            
            // Check if this looks like a path (starts with / or contains path-like characters)
            if trimmed.starts_with('/') || trimmed.contains('/') {
                search_paths.push(PathBuf::from(trimmed));
            } else {
                // If we've started collecting paths and hit a non-path line, we're done
                if !search_paths.is_empty() {
                    break;
                }
            }
        }
    }
    
    if !found_header {
        return Err(MscDiscoveryError::ParseError(
            "Could not find 'Search path for solver configurations:' in output".to_string()
        ));
    }
    
    Ok(search_paths)
}

async fn find_msc_files_in_directory(dir: &Path) -> std::result::Result<Vec<PathBuf>, std::io::Error> {
    let mut msc_files = Vec::new();
    
    if !dir.exists() {
        return Ok(msc_files);
    }
    
    let mut entries = tokio::fs::read_dir(dir).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;
        
        if metadata.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "msc" {
                    msc_files.push(path);
                }
            }
        }
    }
    
    Ok(msc_files)
}

