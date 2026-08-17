use std::time::Duration;

use bevy::prelude::{Color, warn};
use net_contract::events::ViewpointChanged;

use crate::proto::aesir::net;

pub fn viewpoint(m: net::Viewpoint) -> Option<ViewpointChanged> {
    let (r, g, b) = (
        ((m.color >> 16) & 0xFF) as u8,
        ((m.color >> 8) & 0xFF) as u8,
        (m.color & 0xFF) as u8,
    );
    let color = Color::srgb_u8(r, g, b);
    let (x, y) = (m.x as u16, m.y as u16);
    match m.r#type {
        2 => Some(ViewpointChanged::Remove { id: m.id }),
        1 => Some(ViewpointChanged::Show {
            id: m.id,
            x,
            y,
            color,
            ttl: None,
        }),
        0 => Some(ViewpointChanged::Show {
            id: m.id,
            x,
            y,
            color,
            ttl: Some(Duration::from_secs(15)),
        }),
        other => {
            warn!("unsupported Viewpoint type {other}; skipping");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(r#type: u32, id: u32) -> net::Viewpoint {
        net::Viewpoint {
            npc_id: 99,
            r#type,
            x: 10,
            y: 20,
            id,
            color: 0x00FF0000,
        }
    }

    #[test]
    fn maps_supported_types_to_events() {
        let cases = [
            (
                0,
                ViewpointChanged::Show {
                    id: 1,
                    x: 10,
                    y: 20,
                    color: Color::srgb_u8(255, 0, 0),
                    ttl: Some(Duration::from_secs(15)),
                },
            ),
            (
                1,
                ViewpointChanged::Show {
                    id: 1,
                    x: 10,
                    y: 20,
                    color: Color::srgb_u8(255, 0, 0),
                    ttl: None,
                },
            ),
        ];

        for (r#type, expected) in cases {
            let event = viewpoint(marker(r#type, 1)).expect("supported type should map");
            assert_eq!(event, expected);
        }
    }

    #[test]
    fn type_2_removes_the_marker() {
        assert_eq!(
            viewpoint(marker(2, 7)),
            Some(ViewpointChanged::Remove { id: 7 })
        );
    }

    #[test]
    fn unsupported_types_return_none() {
        for r#type in [3, 7, 99, 256] {
            assert_eq!(viewpoint(marker(r#type, 1)), None);
        }
    }

    #[test]
    fn decodes_rgb_and_forces_opaque_alpha() {
        let orange = net::Viewpoint {
            color: 0x00FF8000,
            ..marker(0, 1)
        };
        assert_eq!(
            viewpoint(orange),
            Some(ViewpointChanged::Show {
                id: 1,
                x: 10,
                y: 20,
                color: Color::srgb_u8(255, 128, 0),
                ttl: Some(Duration::from_secs(15)),
            })
        );

        let blue = net::Viewpoint {
            color: 0x000000FF,
            ..marker(0, 1)
        };
        assert_eq!(
            viewpoint(blue),
            Some(ViewpointChanged::Show {
                id: 1,
                x: 10,
                y: 20,
                color: Color::srgb_u8(0, 0, 255),
                ttl: Some(Duration::from_secs(15)),
            })
        );
    }
}
