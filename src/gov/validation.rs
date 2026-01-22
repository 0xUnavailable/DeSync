use crate::types::user_state::{Event, Snapshot, Plot, Task};
use crate::types::gov_types::Rule;

pub fn verify_event(event: &Event, snapshot: &Snapshot, rules: &std::collections::HashMap<String, Rule>) -> bool {
    if let Some(rule) = rules.get(&event.action) {
        let plot = snapshot.land.iter().find(|p| p.plot_id == event.plot_id);
        if plot.is_none() { return false; }
        // TODO: implement full verification logic: energy, grow_duration, deltas
        return true;
    }
    false
}
