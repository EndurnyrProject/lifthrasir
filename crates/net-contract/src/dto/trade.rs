/// Server-to-client reason for ending a player trade or rejecting a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeCancelReason {
    Declined,
    Timeout,
    Cancelled,
    TooFar,
    Busy,
    Dead,
    Disconnected,
    Capacity,
    Invalid,
    Unknown(i32),
}
