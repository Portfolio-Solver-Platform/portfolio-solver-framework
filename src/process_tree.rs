use nix::sys::signal::{self, Signal};
use nix::unistd;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid solver: {0}")]
    KillSolver(String),
}
pub fn recursive_force_kill(root_pid: u32, expected_name: &str) -> Result<()> {
    let system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );

    let root = Pid::from_u32(root_pid);

    // check for safety that the process has the expected name
    if let Some(proc) = system.process(root) {
        let proc_name = proc.name().to_string_lossy();
        if !proc_name.contains(expected_name) && !expected_name.contains(&*proc_name) {
            return Err(Error::KillSolver(format!(
                "SAFETY ABORT: PID {} is active but name '{}' does not match expected '{}'. PID was likely reused.",
                root_pid, proc_name, expected_name,
            )));
        }
    } else {
        return Ok(());
    }

    let mut to_kill = Vec::new();
    collect_descendants(&system, root, &mut to_kill);

    for child_pid in to_kill {
        let _ = signal::kill(
            unistd::Pid::from_raw(child_pid.as_u32() as i32),
            Signal::SIGKILL,
        );
    }

    let _ = signal::kill(unistd::Pid::from_raw(root_pid as i32), Signal::SIGKILL);
    Ok(())
}

fn collect_descendants(system: &System, parent: Pid, acc: &mut Vec<Pid>) {
    for (pid, process) in system.processes() {
        if process.parent() == Some(parent) {
            acc.push(*pid);
            collect_descendants(system, *pid, acc);
        }
    }
}
pub fn get_process_tree_memory(system: &System, root_pid: u32) -> u64 {
    let root_pid = Pid::from_u32(root_pid);
    let mut total_memory = 0u64;
    let mut pids_to_check = vec![root_pid];

    while let Some(pid) = pids_to_check.pop() {
        if let Some(process) = system.process(pid) {
            total_memory += process.memory();
            for (child_pid, child_process) in system.processes() {
                if child_process.parent() == Some(pid) {
                    pids_to_check.push(*child_pid);
                }
            }
        }
    }

    total_memory
}
