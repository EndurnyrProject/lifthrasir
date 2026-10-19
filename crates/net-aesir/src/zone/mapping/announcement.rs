use crate::proto::aesir::net;
use net::announcement::Style;
use net_contract::events::{AnnouncementReceived, AnnouncementStyle};

pub fn announcement(a: net::Announcement) -> AnnouncementReceived {
    let style = match Style::try_from(a.style) {
        Ok(Style::Top) => AnnouncementStyle::Top,
        Ok(Style::Center) => AnnouncementStyle::Center,
        Ok(Style::Local) | Err(_) => AnnouncementStyle::Local,
    };

    AnnouncementReceived {
        text: a.text,
        color: a.color,
        style,
        source_name: a.source_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proto(style: i32) -> net::Announcement {
        net::Announcement {
            text: "hello".into(),
            color: 0x00ff00,
            style,
            source_name: "Alice".into(),
        }
    }

    #[test]
    fn maps_top_style() {
        assert_eq!(
            announcement(proto(Style::Top as i32)).style,
            AnnouncementStyle::Top
        );
    }

    #[test]
    fn maps_center_style() {
        assert_eq!(
            announcement(proto(Style::Center as i32)).style,
            AnnouncementStyle::Center
        );
    }

    #[test]
    fn maps_local_style() {
        assert_eq!(
            announcement(proto(Style::Local as i32)).style,
            AnnouncementStyle::Local
        );
    }

    #[test]
    fn out_of_range_style_falls_back_to_local() {
        assert_eq!(announcement(proto(99)).style, AnnouncementStyle::Local);
    }

    #[test]
    fn passes_fields_through_unchanged() {
        let received = announcement(proto(Style::Center as i32));

        assert_eq!(received.text, "hello");
        assert_eq!(received.color, 0x00ff00);
        assert_eq!(received.source_name, "Alice");
    }
}
