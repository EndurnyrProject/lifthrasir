use bevy::prelude::*;
use bevy_auto_plugin::prelude::auto_add_message;

/// Where a cutin is anchored on the client window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutinPlacement {
    BottomLeft,
    BottomCenter,
    BottomRight,
    CenterWindow,
    CenterChromeless,
}

/// A server-ordered cutin display change: show an illustration or clear the current one.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
#[auto_add_message(plugin = crate::NetContractPlugin)]
pub enum CutinDisplayChanged {
    Show {
        image: String,
        placement: CutinPlacement,
    },
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_placement_constructs_and_compares() {
        let placements = [
            CutinPlacement::BottomLeft,
            CutinPlacement::BottomCenter,
            CutinPlacement::BottomRight,
            CutinPlacement::CenterWindow,
            CutinPlacement::CenterChromeless,
        ];

        for placement in placements {
            assert_eq!(placement, placement);
        }
    }

    #[test]
    fn clear_constructs_and_compares() {
        assert_eq!(CutinDisplayChanged::Clear, CutinDisplayChanged::Clear);
    }

    #[test]
    fn show_carries_its_image_and_placement() {
        let show = CutinDisplayChanged::Show {
            image: "event_illust".to_string(),
            placement: CutinPlacement::CenterWindow,
        };

        assert_eq!(
            show,
            CutinDisplayChanged::Show {
                image: "event_illust".to_string(),
                placement: CutinPlacement::CenterWindow,
            }
        );

        assert_ne!(
            show,
            CutinDisplayChanged::Show {
                image: "other_illust".to_string(),
                placement: CutinPlacement::CenterWindow,
            }
        );

        assert_ne!(
            show,
            CutinDisplayChanged::Show {
                image: "event_illust".to_string(),
                placement: CutinPlacement::BottomLeft,
            }
        );

        assert_ne!(show, CutinDisplayChanged::Clear);
    }
}
