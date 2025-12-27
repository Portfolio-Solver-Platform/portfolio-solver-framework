use nix::sys::signal::{self, Signal};
use nix::unistd;
use std::collections::HashSet;
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

    // // 1. SAFETY CHECK (Uncommented and fixed)
    // // We verify the process exists and matches the expected name to prevent PID reuse accidents.
    // if let Some(proc) = system.process(root) {
    //     let proc_name = proc.name(); // Returns &str in modern sysinfo
    //     if !proc_name.contains(expected_name) && !expected_name.contains(proc_name) {
    //         return Err(Error::KillSolver(format!(
    //             "SAFETY ABORT: PID {} is active but name '{}' does not match expected '{}'. PID was likely reused.",
    //             root_pid, proc_name, expected_name,
    //         )));
    //     }
    // } else {
    //     // Process is already dead!
    //     return Ok(());
    // }

    // Use a Set to ensure uniqueness (prevent double killing)
    let mut pids_to_kill = HashSet::new();

    // 2. STRATEGY A: Collect by Process Group
    // We try to find the PGID of the root.
    if let Some(target_pgid_raw) = get_process_pgid(root_pid) {
        // Cast i32 (kernel) to u32 (sysinfo) for comparison
        let target_pgid = target_pgid_raw as u32;

        for (pid, process) in system.processes() {
            if let Some(pgid) = process.group_id() {
                if *pgid == target_pgid {
                    pids_to_kill.insert(*pid);
                }
            }
        }
    }

    // 3. STRATEGY B: Collect by Tree (Descendants)
    // We add the root itself
    pids_to_kill.insert(root);

    // We also want to find descendants of EVERYONE we found in the group so far.
    // (In case a child in the group spawned a grandchild that detached from the group)
    let current_targets: Vec<Pid> = pids_to_kill.iter().cloned().collect();
    for target in current_targets {
        collect_descendants(&system, target, &mut pids_to_kill);
    }

    // 4. EXECUTE
    // We kill the children/group members first
    for pid in &pids_to_kill {
        // Don't kill the root just yet, save it for last
        if *pid == root {
            continue;
        }

        let _ = signal::kill(unistd::Pid::from_raw(pid.as_u32() as i32), Signal::SIGKILL);
    }

    // Finally kill the root
    let _ = signal::kill(unistd::Pid::from_raw(root_pid as i32), Signal::SIGKILL);

    Ok(())
}

fn collect_descendants(system: &System, parent: Pid, acc: &mut HashSet<Pid>) {
    for (pid, process) in system.processes() {
        if process.parent() == Some(parent) {
            // If we haven't seen this child yet, add it and recurse
            if acc.insert(*pid) {
                collect_descendants(system, *pid, acc);
            }
        }
    }
}

pub fn get_process_pgid(pid: u32) -> Option<i32> {
    let pid_wrapper = nix::unistd::Pid::from_raw(pid as i32);
    match unistd::getpgid(Some(pid_wrapper)) {
        Ok(pgid) => Some(pgid.as_raw()),
        Err(_) => None,
    }
}

pub fn get_pids_in_group(target_pgid: u32) -> Vec<u32> {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::everything()),
    );

    let mut group_members = Vec::new();

    for (pid, process) in system.processes() {
        if let Some(gid) = process.group_id() {
            if *gid == target_pgid {
                group_members.push(pid.as_u32());
            }
        }
    }

    group_members
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
