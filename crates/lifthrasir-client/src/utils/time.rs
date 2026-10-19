use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current time in milliseconds since UNIX epoch
///
/// This function is used for client-server time synchronization.
/// The server uses millisecond precision for movement timing and other events.
///
/// # Returns
///
/// Current time in milliseconds as u32 (wraps around every ~49.7 days)
pub fn current_milliseconds() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_milliseconds() {
        let time1 = current_milliseconds();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let time2 = current_milliseconds();

        assert!(time2 >= time1, "Time should be monotonically increasing");
        assert!(time2 - time1 >= 10, "At least 10ms should have elapsed");
    }
}
