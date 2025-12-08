use std::collections::{HashMap, HashSet};

use sysinfo::{Pid, System};

use crate::{
    args::Args,
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

pub struct Scheduler {
    running_solvers: HashMap<usize, (String, usize)>,
    suspended_solvers: HashMap<usize, (String, usize)>,

    solver_manager: SolverManager,
}

impl Scheduler {
    pub fn new(args: &Args) -> std::result::Result<Self, Error> {
        let solver_manager = SolverManager::new(args.clone())?;

        Ok(Self {
            running_solvers: HashMap::new(),
            suspended_solvers: HashMap::new(),
            solver_manager,
        })
    }

    pub async fn apply(&mut self, schedule: Schedule) -> std::result::Result<(), Vec<Error>> {
        let mut new_solvers = HashSet::new();
        for elem in &schedule {
            new_solvers.insert(elem.solver.clone());
        }
        let mut current_solvers = HashSet::new();
        for (_, (solver, _)) in self.running_solvers.iter() {
            current_solvers.insert(solver);
        }

        let mut solvers_to_suspend: Vec<usize> = Vec::new();
        for (id, (name, _)) in self.running_solvers.iter() {
            if !new_solvers.contains(name) {
                solvers_to_suspend.push(*id);
            }
        }
        let mut solvers_to_start: Schedule = Vec::new();

        for elem in schedule.into_iter() {
            if !current_solvers.contains(&elem.solver) {
                solvers_to_start.push(elem);
            }
        }
        // println!("{:?}", solvers_to_start);
        self.solver_manager
            .suspend_solvers(solvers_to_suspend)
            .await?;

        for elem in solvers_to_start.iter() {
            self.running_solvers
                .insert(elem.id, (elem.solver.clone(), elem.cores));
        }

        self.solver_manager.start_solvers(solvers_to_start).await
    }

    pub async fn apply2(&mut self, schedule: Schedule) -> std::result::Result<(), Vec<Error>> {
        let mut solvers_to_start: Schedule = Vec::new();
        let mut solvers_to_suspend: Vec<usize> = Vec::new();
        let mut solvers_to_resume: Vec<usize> = Vec::new();
        let mut solvers_to_stop: Vec<usize> = Vec::new();

        let mut running_solvers = HashSet::new();
        for id in self.running_solvers.keys() {
            running_solvers.insert(*id);
        }

        let mut suspended_solvers = HashSet::new();
        for id in self.suspended_solvers.keys() {
            suspended_solvers.insert(*id);
        }

        for elem in schedule {
            if running_solvers.contains(&elem.id) {
                running_solvers.remove(&elem.id); // remove so we dont suspend it later
            } else if suspended_solvers.contains(&elem.id) {
                self.running_solvers
                    .insert(elem.id, (elem.solver.clone(), elem.cores));
                self.suspended_solvers.remove(&elem.id);
                suspended_solvers.remove(&elem.id);
                solvers_to_resume.push(elem.id);
            } else {
                self.running_solvers
                    .insert(elem.id, (elem.solver.clone(), elem.cores));
                solvers_to_start.push(elem);
            }
        }

        for id in solvers_to_stop.iter() {
            if self.running_solvers.contains_key(id) {
                self.running_solvers.remove(id);
            } else {
                self.suspended_solvers.remove(id);
            }
        }

        self.solver_manager.stop_solvers(solvers_to_stop).await?;

        // suspend remaining solvers
        for id in running_solvers {
            solvers_to_suspend.push(id);
        }

        for id in solvers_to_suspend.iter() {
            let (solver, cores) = self.running_solvers.remove(id).unwrap(); // should never fail, otherwise there is a logic error in code
            let elem = ScheduleElement::new(*id, solver, cores);
            self.suspended_solvers
                .insert(elem.id, (elem.solver.clone(), elem.cores));
        }

        self.solver_manager
            .suspend_solvers(solvers_to_suspend)
            .await?;

        self.solver_manager
            .resume_solvers(solvers_to_resume)
            .await?;

        self.solver_manager.start_solvers(solvers_to_start).await
    }
}

fn get_process_memory(system: &mut System, pid: u32) -> Option<u64> {
    let pid = Pid::from_u32(pid);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), false);
    system.process(pid).map(|p| p.memory())
}
