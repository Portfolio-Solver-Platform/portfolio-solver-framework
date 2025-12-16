use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    pub dynamic_schedule_interval: u64,
    pub memory_enforcer_interval: u64,
    pub memory_threshold: f64,
    pub solver_args: HashMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        let mut solver_args = HashMap::new();
        // Default args for most solvers
        let default_args = vec!["-i".to_string(), "-f".to_string()];
        solver_args.insert("gecode".to_string(), default_args.clone());
        solver_args.insert("chuffed".to_string(), default_args.clone());
        solver_args.insert("coinbc".to_string(), default_args.clone());
        solver_args.insert("cp-sat".to_string(), default_args.clone());
        solver_args.insert("yuck".to_string(), default_args.clone());
        // Picat doesn't support -i flag
        solver_args.insert(
            "picat".to_string(),
            vec!["-a".to_string(), "-f".to_string()],
        );

        Self {
            dynamic_schedule_interval: 5,
            memory_enforcer_interval: 3,
            memory_threshold: 0.9,
            solver_args,
        }
    }
}
