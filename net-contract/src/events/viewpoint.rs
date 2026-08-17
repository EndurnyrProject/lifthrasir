use std::time::Duration;

use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Server-driven minimap/compass marker (rAthena `viewpoint`, replaces ZC_COMPASS).
#[derive(Message, Debug, Clone, PartialEq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub enum ViewpointChanged {
    /// Show or replace the marker in slot `id` at cell `(x, y)`.
    /// `ttl = Some(15s)` for the timed type; `None` persists until map change.
    Show {
        id: u32,
        x: u16,
        y: u16,
        color: Color,
        ttl: Option<Duration>,
    },
    /// Remove the marker in slot `id`.
    Remove { id: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_constructs_and_compares() {
        let show = ViewpointChanged::Show {
            id: 7,
            x: 100,
            y: 200,
            color: Color::srgb_u8(255, 0, 0),
            ttl: Some(Duration::from_secs(15)),
        };

        assert_eq!(
            show,
            ViewpointChanged::Show {
                id: 7,
                x: 100,
                y: 200,
                color: Color::srgb_u8(255, 0, 0),
                ttl: Some(Duration::from_secs(15)),
            }
        );

        assert_ne!(
            show,
            ViewpointChanged::Show {
                id: 8,
                x: 100,
                y: 200,
                color: Color::srgb_u8(255, 0, 0),
                ttl: Some(Duration::from_secs(15)),
            }
        );

        assert_ne!(
            show,
            ViewpointChanged::Show {
                id: 7,
                x: 100,
                y: 200,
                color: Color::srgb_u8(0, 255, 0),
                ttl: Some(Duration::from_secs(15)),
            }
        );

        assert_ne!(
            show,
            ViewpointChanged::Show {
                id: 7,
                x: 100,
                y: 200,
                color: Color::srgb_u8(255, 0, 0),
                ttl: None,
            }
        );

        assert_ne!(show, ViewpointChanged::Remove { id: 7 });
    }

    #[test]
    fn remove_constructs_and_compares() {
        assert_eq!(
            ViewpointChanged::Remove { id: 3 },
            ViewpointChanged::Remove { id: 3 }
        );
        assert_ne!(
            ViewpointChanged::Remove { id: 3 },
            ViewpointChanged::Remove { id: 4 }
        );
    }
}
