use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use sysinfo::System;
use tokio::sync::Mutex;

use crate::{
    args::{Args, DebugVerbosityLevel},
    config::Config,
    solver_manager::{Error, SolverManager},
};

#[derive(Clone, Debug)]
pub struct ScheduleElement {
    pub id: usize,
    pub solver: String,
    pub cores: usize,
}

impl ScheduleElement {
    pub fn new(id: usize, solver: String, cores: usize) -> Self {
        Self { id, solver, cores }
    }
}

pub type Schedule = Vec<ScheduleElement>;

struct SolverInfo {
    name: String,
    cores: usize,
}

struct ScheduleChanges {
    to_start: Schedule,
    to_suspend: Vec<usize>,
    to_resume: Vec<usize>,
    to_stop: Vec<usize>,
}

struct MemoryEnforcerState {
    running_solvers: HashMap<usize, SolverInfo>,
    suspended_solvers: HashMap<usize, SolverInfo>,
    system: System,
    memory_threshold: f64,
    memory_limit: u64, // In bytes (0 = use system total)
}

pub struct Scheduler {
    state: Arc<Mutex<MemoryEnforcerState>>,
    pub solver_manager: Arc<SolverManager>,
}

fn is_over_threshold(used: f64, total: f64, threshold: f64) -> bool {
    used / total > threshold
}

impl Scheduler {
    pub fn new(args: &Args, config: &Config) -> std::result::Result<Self, Error> {
        let solver_manager = Arc::new(SolverManager::new(args.clone())?);

        let memory_limit = std::env::var("MEMORY_LIMIT")
            .ok()
            .and_then(|val| val.parse::<u64>().ok())
            .map(|mib| mib * 1024 * 1024)
            .unwrap_or(0);

        let state = Arc::new(Mutex::new(MemoryEnforcerState {
            running_solvers: HashMap::new(),
            suspended_solvers: HashMap::new(),
            system: System::new_all(),
            memory_threshold: config.memory_threshold,
            memory_limit,
        }));

        let state_clone = state.clone();
        let solver_manager_clone = solver_manager.clone();
        let debug_verbosity = args.debug_verbosity;
        let memory_enforcer_interval = config.memory_enforcer_interval;
        tokio::spawn(async move {
            Self::memory_enforcer_loop(
                state_clone,
                solver_manager_clone,
                debug_verbosity,
                memory_enforcer_interval,
            )
            .await;
        });

        Ok(Self {
            state,
            solver_manager,
        })
    }

    fn get_memory_usage(state: &mut MemoryEnforcerState) -> (f64, f64) {
        state
            .system
            .refresh_processes(sysinfo::ProcessesToUpdate::All, false);
        state.system.refresh_memory();

        let used = state.system.used_memory() as f64;
        let total = if state.memory_limit > 0 {
            state.memory_limit as f64
        } else {
            state.system.total_memory() as f64
        };
        (used, total)
    }

    async fn kill_suspended_until_under_threshold(
        state: &mut MemoryEnforcerState,
        solver_manager: &Arc<SolverManager>,
        mut used_memory: f64,
        total_memory: f64,
        debug_verbosity: DebugVerbosityLevel,
    ) -> f64 {
        let ids: Vec<usize> = state.suspended_solvers.keys().copied().collect();
        let mut sorted = solver_manager
            .solvers_sorted_by_mem(&ids, &state.system)
            .await;

        while !sorted.is_empty()
            && is_over_threshold(used_memory, total_memory, state.memory_threshold)
        {
            let (mem, id) = sorted.remove(0);
            state.suspended_solvers.remove(&id);
            if let Err(e) = solver_manager.stop_solver(id).await {
                if debug_verbosity >= DebugVerbosityLevel::Error {
                    eprintln!("failed to stop suspended solver: {e}");
                }
            } else {
                used_memory -= mem as f64;
            }
        }
        used_memory
    }

    async fn kill_running_until_under_threshold(
        state: &mut MemoryEnforcerState,
        solver_manager: &Arc<SolverManager>,
        mut used_memory: f64,
        total_memory: f64,
        debug_verbosity: DebugVerbosityLevel,
    ) -> f64 {
        let ids: Vec<usize> = state.running_solvers.keys().copied().collect();
        let num_running = ids.len();
        if num_running == 0 {
            return used_memory;
        }

        let sorted = solver_manager
            .solvers_sorted_by_mem(&ids, &state.system)
            .await;
        let per_solver_threshold =
            (total_memory / num_running as f64 * state.memory_threshold) as u64;

        for (solver_mem, id) in sorted {
            if solver_mem <= per_solver_threshold
                || !is_over_threshold(used_memory, total_memory, state.memory_threshold)
            {
                break;
            }
            state.running_solvers.remove(&id);
            if let Err(e) = solver_manager.stop_solver(id).await {
                if debug_verbosity >= DebugVerbosityLevel::Error {
                    eprintln!("failed to stop running solver: {e}");
                }
            } else {
                used_memory -= solver_mem as f64;
            }
        }
        used_memory
    }

    async fn remove_exited_solvers(
        state: &mut MemoryEnforcerState,
        solver_manager: &Arc<SolverManager>,
    ) {
        let active = solver_manager.active_solver_ids().await;
        state.running_solvers.retain(|id, _| active.contains(id));
        state.suspended_solvers.retain(|id, _| active.contains(id));
    }

    async fn memory_enforcer_loop(
        state: Arc<Mutex<MemoryEnforcerState>>,
        solver_manager: Arc<SolverManager>,
        debug_verbosity: DebugVerbosityLevel,
        memory_enforcer_interval: u64,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(memory_enforcer_interval));

        loop {
            interval.tick().await;
            let mut state = state.lock().await;

            Self::remove_exited_solvers(&mut state, &solver_manager).await;

            let (used, total) = Self::get_memory_usage(&mut state);
            if !is_over_threshold(used, total, state.memory_threshold) {
                continue;
            }
            // println!()
            let used = Self::kill_suspended_until_under_threshold(
                &mut state,
                &solver_manager,
                used,
                total,
                debug_verbosity,
            )
            .await;

            if is_over_threshold(used, total, state.memory_threshold) {
                Self::kill_running_until_under_threshold(
                    &mut state,
                    &solver_manager,
                    used,
                    total,
                    debug_verbosity,
                )
                .await;
            }
        }
    }

    async fn categorize_schedule(
        schedule: Schedule,
        state: &mut MemoryEnforcerState,
        solver_manager: Arc<SolverManager>,
    ) -> ScheduleChanges {
        Self::remove_exited_solvers(state, &solver_manager).await;

        let mut to_start = Vec::new();
        let mut to_resume = Vec::new();
        let mut to_stop = Vec::new();
        let mut keep_running = Vec::new();
        let mut running: HashSet<_> = state.running_solvers.keys().copied().collect();
        let mut suspended: HashSet<_> = state.suspended_solvers.keys().copied().collect();

        let (used, total) = Self::get_memory_usage(state);
        let _is_over_threshold = is_over_threshold(used, total, state.memory_threshold);

        for elem in schedule {
            if running.remove(&elem.id) {
                // already running, keep it running
                keep_running.push(elem.id);
            } else if suspended.remove(&elem.id) {
                to_resume.push(elem.id);
            } else {
                to_start.push(elem);
            }
        }

        // if we are over limit dont start more solvers, however always make sure at least one is running
        if _is_over_threshold && to_resume.len() + to_start.len() + keep_running.len() > 1 {
            if keep_running.len() >= 1 {
                let solvers_sorted_by_mem = solver_manager
                    .solvers_sorted_by_mem(&keep_running, &state.system)
                    .await;
                let len = solvers_sorted_by_mem.len();
                for (_, id) in solvers_sorted_by_mem.iter().take(len.saturating_sub(1)) {
                    // let always one solver running
                    to_stop.push(*id);
                }

                to_resume = Vec::new();
                to_start = Vec::new();
            } else if to_resume.len() >= 1 {
                to_resume = vec![to_resume[0]]; // start only the first
                to_start = Vec::new();
            } else {
                to_start = vec![to_start[0].clone()]; // start only the first
            }
        }
        let to_suspend = running.into_iter().collect();

        ScheduleChanges {
            to_start,
            to_suspend,
            to_resume,
            to_stop,
        }
    }

    fn apply_changes_to_state(state: &mut MemoryEnforcerState, changes: &ScheduleChanges) {
        for elem in &changes.to_start {
            state.running_solvers.insert(
                elem.id,
                SolverInfo {
                    name: elem.solver.clone(),
                    cores: elem.cores,
                },
            );
        }

        for id in &changes.to_stop {
            state.running_solvers.remove(&id);
        }

        for &id in &changes.to_resume {
            if let Some(info) = state.suspended_solvers.remove(&id) {
                state.running_solvers.insert(id, info);
            }
        }

        for &id in &changes.to_suspend {
            if let Some(info) = state.running_solvers.remove(&id) {
                state.suspended_solvers.insert(id, info);
            }
        }
    }

    pub async fn apply(&mut self, schedule: Schedule) -> std::result::Result<(), Vec<Error>> {
        let mut state = self.state.lock().await;
        let changes =
            Self::categorize_schedule(schedule, &mut state, self.solver_manager.clone()).await;
        Self::apply_changes_to_state(&mut state, &changes);

        self.solver_manager.stop_solvers(changes.to_stop).await?;
        self.solver_manager
            .suspend_solvers(changes.to_suspend)
            .await?;
        self.solver_manager
            .resume_solvers(changes.to_resume)
            .await?;
        self.solver_manager.start_solvers(changes.to_start).await
    }
}
