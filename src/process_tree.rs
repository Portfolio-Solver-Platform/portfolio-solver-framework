use sysinfo::{Pid, System};

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
