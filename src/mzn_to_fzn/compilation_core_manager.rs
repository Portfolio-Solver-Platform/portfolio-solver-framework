use tokio::sync::RwLock;

use super::compilation_manager::CompilationManager;
use crate::{
    args::RunArgs,
    is_cancelled::IsCancelled,
    logging,
    mzn_to_fzn::compilation_manager::{CompilationStatus, WaitForResult},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

// TODO: Find better name
pub struct CompilationCoreManager {
    manager: Arc<CompilationManager>,
    queue: Arc<RwLock<CompilationPriority>>,
}

// General procedure:
//
// Create a CompilationPriority struct that manages which compilation
// should be done next. The next compilations should be stored in a btree.
//
// Start the solver through the self.manager.
// If cores > 1, then start cores - 1 compilations and register these
// in the CompilationPriority struct.
// Then, start threads that wait for the extra compilations.
//  - In these threads, when it is done, it should start a new one
//    by registering the compilation as done in the CompilationPriority
//    and then starting a new compilation.
// Then, wait for the compilation.
// Then, stop cores - 1 compilations.
//
// TODO: Handle main compilations should not be able to be stopped.
//       Currently, they can be stopped when another main compilation is finished, and it has low
//       priority.
impl CompilationCoreManager {
    pub fn new(args: Arc<RunArgs>, compilation_priorities: Vec<String>) -> Self {
        let queue = CompilationPriority::from_vec(compilation_priorities);
        Self {
            manager: Arc::new(CompilationManager::new(args)),
            queue: Arc::new(RwLock::new(queue)),
        }
    }

    pub async fn start(&self, solver_id: String, cores: u64) {
        match self.manager.status(&solver_id).await {
            // Ignore compilations that are done or running
            CompilationStatus::Done | CompilationStatus::Running => {
                logging::info!(
                    "did not start the compilation of solver '{solver_id}' because it was already done or running",
                );
                return;
            }
            CompilationStatus::NotStarted => {}
        }

        let solver_id = SolverId(solver_id);
        self.queue.write().await.take_to_start(&solver_id);
        self.manager.start(solver_id.to_string()).await;

        let extra_compilations_count = cores - 1;
        let extra_compilations = self
            .queue
            .write()
            .await
            .take_next_to_start(extra_compilations_count);
        for solver_id in extra_compilations {
            let manager = Arc::clone(&self.manager);
            let queue = Arc::clone(&self.queue);
            tokio::spawn(async move {
                Self::extra_compilation(manager, queue, solver_id).await;
            });
        }

        let manager = Arc::clone(&self.manager);
        let queue = Arc::clone(&self.queue);
        tokio::spawn(async move {
            Self::main_compilation(manager, queue, solver_id, extra_compilations_count).await;
        });
    }

    pub fn stop(&self, solver_id: &str) {
        todo!()
    }

    pub async fn stop_all_except(&self, exception_solver_ids: HashSet<String>) {
        todo!()
    }

    pub async fn wait_for(&self, solver_name: &str) -> WaitForResult {
        todo!()
    }

    async fn extra_compilation(
        manager: Arc<CompilationManager>,
        queue: Arc<RwLock<CompilationPriority>>,
        mut solver_id: SolverId,
    ) {
        loop {
            manager.start(solver_id.0.clone()).await;
            let result = manager.wait_for(&solver_id.0).await;

            if let Err(error) = result
                && error.is_cancelled()
            {
                queue.write().await.set_stopped(&solver_id);
                return;
            }

            // It might have failed, but we don't want to repeat it so we still mark it as done
            let mut queue = queue.write().await;
            queue.set_done(&solver_id);

            let compilations = queue.take_next_to_start(1);
            if compilations.len() >= 2 {
                logging::error_msg!("take_next_to_start returned more than 1");
            }

            if let Some(new_solver_id) = compilations.into_iter().next() {
                solver_id = new_solver_id;
            } else {
                break;
            }
        }
    }

    /// Precondition: the solver has been started in the self.queue.
    async fn main_compilation(
        manager: Arc<CompilationManager>,
        queue: Arc<RwLock<CompilationPriority>>,
        solver_id: SolverId,
        extra_compilations_count: u64,
    ) {
        // We are not interested in the result because we in either case
        // want to stop the extra compilations.
        let _ = manager.wait_for(&solver_id.to_string()).await;

        // and stops the extra compilations when done.
        let compilations_to_stop = queue
            .write()
            .await
            .take_next_to_stop(extra_compilations_count);

        manager
            .stop_many(compilations_to_stop.into_iter().map(|id| id.0))
            .await;
    }
}

struct SolverId(String);
#[derive(PartialOrd, PartialEq, Eq, Ord)]
struct Priority(u64);

struct CompilationPriority {
    to_start_queue: BTreeMap<Priority, SolverId>,
    running: HashMap<SolverId, Priority>,
}

impl CompilationPriority {
    pub fn from_vec(solvers: Vec<String>) -> Self {
        let priorities = solvers
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, solver_id)| (Priority(index as u64), SolverId(solver_id)));

        Self {
            to_start_queue: BTreeMap::from_iter(priorities),
            running: Default::default(),
        }
    }

    /// Assumes the returned compilations are started after this call
    pub fn take_next_to_start(&mut self, count: u64) -> Vec<SolverId> {
        todo!()
    }

    /// Assumes the returned compilations are stopped after this call
    pub fn take_next_to_stop(&mut self, count: u64) -> Vec<SolverId> {
        todo!()
    }

    /// Assumes the solver is started after this call
    pub fn take_to_start(&mut self, solver: &SolverId) {
        todo!()
    }

    pub fn set_done(&mut self, solver: &SolverId) {
        todo!()
    }

    pub fn set_stopped(&mut self, solver: &SolverId) {
        todo!()
    }
}

impl ToString for SolverId {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl SolverId {
    pub fn into_string(self) -> String {
        self.0
    }
}
