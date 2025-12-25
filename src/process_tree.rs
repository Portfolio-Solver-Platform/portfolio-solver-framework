use nix::sys::signal::{self, Signal};
use nix::unistd;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

pub fn send_signal_to_tree(pid: u32, signal: Signal) {
    let system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    );

    let mut descendants = Vec::new();
    collect_descendants(&system, Pid::from_u32(pid), &mut descendants);

    // Send signal to descendants first (children before parents)
    for &child_pid in descendants.iter().rev() {
        let _ = signal::kill(unistd::Pid::from_raw(child_pid.as_u32() as i32), signal);
    }

    let _ = signal::kill(unistd::Pid::from_raw(pid as i32), signal);
}

pub fn collect_descendants(system: &System, parent_pid: Pid, descendants: &mut Vec<Pid>) {
    for (pid, process) in system.processes() {
        if process.parent() == Some(parent_pid) {
            descendants.push(*pid);
            collect_descendants(system, *pid, descendants);
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
