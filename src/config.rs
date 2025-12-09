pub struct Config {
    pub dynamic_schedule_interval: u64,
    pub memory_enforcer_interval: u64,
    pub memory_threshold: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dynamic_schedule_interval: 5,
            memory_enforcer_interval: 1,
            memory_threshold: 0.8,
        }
    }
}
