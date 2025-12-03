pub struct ScheduleElement {
    pub solver: String,
    pub cores: usize,
}

impl ScheduleElement {
    fn new(solver: String, cores: usize) -> Self {
        Self { solver, cores }
    }
}

pub fn ai(cores: usize) -> Vec<ScheduleElement> {
    vec![
        ScheduleElement::new("gecode".to_string(), cores / 2),
        ScheduleElement::new("coinbc".to_string(), cores / 2),
    ]
}
