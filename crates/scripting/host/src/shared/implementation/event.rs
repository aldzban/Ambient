use crate::shared::EventSharedState;
use elements_ecs::EntityData;

pub fn subscribe(shared_state: &mut EventSharedState, name: &str) {
    shared_state.subscribed_events.insert(name.to_string());
}

pub fn send(shared_state: &mut EventSharedState, name: &str, data: EntityData) {
    if name.starts_with("dims/") {
        return;
    }
    shared_state.events.push((name.to_string(), data));
}
