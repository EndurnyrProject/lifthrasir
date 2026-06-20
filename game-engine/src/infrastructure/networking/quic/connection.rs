use bevy::log::{debug, warn};
use bevy_quinnet::client::{connection::ClientSideConnection, ClientSendError};

use super::{
    channels,
    envelope::{self, Body},
};

#[derive(Default)]
pub struct QuicConnection {
    seq: u32,
}

impl QuicConnection {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn next_frame(&mut self, body: Body) -> bytes::Bytes {
        let frame = envelope::encode(self.seq, body);
        self.seq += 1;
        frame
    }

    pub fn send(
        &mut self,
        conn: &mut ClientSideConnection,
        channel: u8,
        body: Body,
    ) -> Result<(), ClientSendError> {
        let payload = self.next_frame(body);
        conn.send_payload_on(channel, payload)
    }

    pub fn drain(conn: &mut ClientSideConnection) -> Vec<(u8, Body)> {
        let all_channels = [
            channels::CONTROL,
            channels::GAMEPLAY,
            channels::WORLD,
            channels::BULK,
            channels::SNAPSHOTS,
        ];
        let mut out = Vec::new();
        for ch in all_channels {
            loop {
                match conn.receive_payload(ch) {
                    Ok(Some(bytes)) => match envelope::decode(&bytes) {
                        Ok(env) => match env.body {
                            Some(body) => out.push((ch, body)),
                            None => warn!("received envelope with no body on channel {ch}"),
                        },
                        Err(e) => warn!("failed to decode envelope on channel {ch}: {e}"),
                    },
                    Ok(None) => break,
                    Err(e) => {
                        debug!("receive_payload closed on channel {ch}: {e}");
                        break;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::networking::quic::{
        envelope,
        proto::aesir::net::{Hello, LoginRequest},
    };

    #[test]
    fn seq_increments_across_frames() {
        let mut conn = QuicConnection::new();

        let body0 = Body::Hello(Hello {
            protocol_version: 1,
            build: "test".into(),
        });
        let body1 = Body::LoginRequest(LoginRequest {
            username: "user".into(),
            password: "pass".into(),
            client_version: 1,
        });

        let frame0 = conn.next_frame(body0.clone());
        let frame1 = conn.next_frame(body1.clone());

        let env0 = envelope::decode(&frame0).expect("decode frame0");
        let env1 = envelope::decode(&frame1).expect("decode frame1");

        assert_eq!(env0.seq, 0);
        assert_eq!(env1.seq, 1);
        assert_eq!(env0.body, Some(body0));
        assert_eq!(env1.body, Some(body1));
    }

    #[test]
    fn decode_garbage_errors() {
        assert!(envelope::decode(b"not a protobuf").is_err());
    }
}
