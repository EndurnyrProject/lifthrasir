use bevy::prelude::*;

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct WalkablePath {
    pub waypoints: Vec<(u16, u16)>,
    pub current_waypoint: usize,
    pub final_destination: (u16, u16),
}

impl WalkablePath {
    pub fn new(waypoints: Vec<(u16, u16)>, final_destination: (u16, u16)) -> Self {
        Self {
            waypoints,
            current_waypoint: 0,
            final_destination,
        }
    }

    pub fn new_at_waypoint(
        waypoints: Vec<(u16, u16)>,
        final_destination: (u16, u16),
        start_index: usize,
    ) -> Self {
        Self {
            waypoints,
            current_waypoint: start_index,
            final_destination,
        }
    }

    pub fn next_waypoint(&self) -> Option<(u16, u16)> {
        self.waypoints.get(self.current_waypoint).copied()
    }

    pub fn advance(&mut self) -> bool {
        if self.current_waypoint + 1 < self.waypoints.len() {
            self.current_waypoint += 1;
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_waypoint + 1 >= self.waypoints.len()
    }
}
