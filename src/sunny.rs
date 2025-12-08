use crate::fzn_to_features::fzn_to_features;
use crate::mzn_to_fzn::convert_mzn_to_fzn;
use crate::scheduler::{Schedule, ScheduleElement};
use crate::{ai::Ai, scheduler::Scheduler};
use tokio::time::{Duration, sleep};

use crate::{args::Args, solver_manager::SolverManager};

const FEATURES_SOLVER: &str = "gecode";

pub async fn sunny(args: Args, mut ai: impl Ai, dynamic_schedule_interval: u64) {
    let timer_duration = Duration::from_secs(dynamic_schedule_interval);
    let cores = args.cores.unwrap_or(2);
    let mut solver_manager = Scheduler::new(&args).expect("Failed to create solver_manager");
    solver_manager.apply(static_schedule(cores)).await.unwrap(); // TODO: Maybe do this in another thread

    let mut timer = sleep(timer_duration);
    let fzn = convert_mzn_to_fzn(
        &args.model,
        args.data.as_deref(),
        FEATURES_SOLVER,
        args.debug_verbosity,
    )
    .await
    .expect("failed to initially convert .mzn to .fzn");
    // let features = fzn_to_features(&fzn)
    //     .await
    //     .expect("if we fail to get features, we can't run the AI and thus can't recover");

    loop {
        timer.await;
        let schedule = ai.schedule(&vec![], cores);
        solver_manager.apply(schedule).await.unwrap();
        // apply_schedule(&mut solver_manager, schedule).await;
        // solver_manager.stop_all_solvers().await.unwrap();

        // solver_manager.suspend_all_solvers().await.unwrap();
        // solver_manager.resume_all_solvers().await.unwrap();

        // solver_manager.resume_solver(1).await.unwrap();
        // solver_manager.resume_solver(2).await.unwrap();
        // solver_manager.resume_solver(1).await.unwrap();
        // solver_manager.resume_solver(1).await.unwrap();
        // solver_manager.resume_solver(1).await.unwrap();

        timer = sleep(timer_duration);
    }
}

fn static_schedule(cores: usize) -> Schedule {
    // let solvers = vec![
    //     "picat".to_string(),
    //     "gecode".to_string(),
    //     "cp-sat".to_string(),
    //     "chuffed".to_string(),
    //     "coinbc".to_string(),
    //     "yuck".to_string(),
    // ];
    // let mut schedule = vec![];
    // for (i, solver) in solvers.into_iter().cycle().take(100).enumerate() {
    //     schedule.push(ScheduleElement::new(i, solver, 1));
    // }
    // schedule
    vec![
        ScheduleElement::new(0, "picat".to_string(), 1),
        ScheduleElement::new(1, "gecode".to_string(), 1),
        ScheduleElement::new(2, "cp-sat".to_string(), 1),
        ScheduleElement::new(3, "chuffed".to_string(), 1),
        ScheduleElement::new(4, "coinbc".to_string(), 1),
        ScheduleElement::new(5, "yuck".to_string(), 1),
        // ScheduleElement::new( "xpress".to_string(), cores / 10),
        // ScheduleElement::new( "scip".to_string(), cores / 10),
        // ScheduleElement::new( "highs".to_string(), cores / 10),
        // ScheduleElement::new( "gurobi".to_string(), cores / 10),
        // ScheduleElement::new("coinbc".to_string(), cores / 2),
    ]
}
