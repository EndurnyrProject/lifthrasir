use bevy::prelude::warn;
use net_contract::events::{CutinDisplayChanged, CutinPlacement};

use crate::proto::aesir::net;

pub fn cutin(message: net::Cutin) -> Option<CutinDisplayChanged> {
    let placement = match message.r#type {
        0 => CutinPlacement::BottomLeft,
        1 => CutinPlacement::BottomCenter,
        2 => CutinPlacement::BottomRight,
        3 => CutinPlacement::CenterWindow,
        4 => CutinPlacement::CenterChromeless,
        255 => return Some(CutinDisplayChanged::Clear),
        unknown => {
            warn!(
                "unsupported Cutin type {unknown}; skipping '{}'",
                message.image
            );
            return None;
        }
    };

    Some(CutinDisplayChanged::Show {
        image: message.image,
        placement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_supported_types_to_placements() {
        let cases = [
            (0, CutinPlacement::BottomLeft),
            (1, CutinPlacement::BottomCenter),
            (2, CutinPlacement::BottomRight),
            (3, CutinPlacement::CenterWindow),
            (4, CutinPlacement::CenterChromeless),
        ];

        for (r#type, expected) in cases {
            let event = cutin(net::Cutin {
                image: "일러스트_01".into(),
                r#type,
            })
            .expect("supported type should map");

            assert_eq!(
                event,
                CutinDisplayChanged::Show {
                    image: "일러스트_01".into(),
                    placement: expected,
                }
            );
        }
    }

    #[test]
    fn type_255_clears_regardless_of_image() {
        for image in ["", "stale_illust.bmp", "일러스트"] {
            assert_eq!(
                cutin(net::Cutin {
                    image: image.into(),
                    r#type: 255,
                }),
                Some(CutinDisplayChanged::Clear)
            );
        }
    }

    #[test]
    fn unsupported_types_return_none() {
        for r#type in [5, 6, 99, 256] {
            assert_eq!(
                cutin(net::Cutin {
                    image: "event_illust".into(),
                    r#type,
                }),
                None
            );
        }
    }
}
