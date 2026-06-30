use bytes::Bytes;
use prost::Message;

use crate::proto::aesir::net::{envelope, Envelope};

pub use envelope::Body;

pub fn encode(seq: u32, body: Body) -> Bytes {
    Bytes::from(
        Envelope {
            seq,
            body: Some(body),
        }
        .encode_to_vec(),
    )
}

pub fn decode(bytes: &[u8]) -> Result<Envelope, prost::DecodeError> {
    Envelope::decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::aesir::net::{
        CharServerInfo, Hello, LoginFailed, LoginRequest, LoginResponse,
    };

    fn roundtrip(seq: u32, body: Body) -> Envelope {
        let encoded = encode(seq, body);
        decode(&encoded).expect("decode failed")
    }

    #[test]
    fn hello_roundtrip() {
        let body = Body::Hello(Hello {
            protocol_version: 1,
            build: "test-build".into(),
        });
        let env = roundtrip(42, body.clone());
        assert_eq!(env.seq, 42);
        assert_eq!(env.body, Some(body));
    }

    #[test]
    fn login_request_roundtrip() {
        let body = Body::LoginRequest(LoginRequest {
            username: "player".into(),
            password: "secret".into(),
            client_version: 20241101,
        });
        let env = roundtrip(1, body.clone());
        assert_eq!(env.seq, 1);
        assert_eq!(env.body, Some(body));
    }

    #[test]
    fn login_response_roundtrip() {
        let body = Body::LoginResponse(LoginResponse {
            account_id: 2000001,
            login_id1: 111,
            login_id2: 222,
            sex: 1,
            auth_token: "abc123".into(),
            char_servers: vec![CharServerInfo {
                name: "Midgard".into(),
                ip: "127.0.0.1".into(),
                port: 6121,
                user_count: 5,
                server_type: 0,
                is_new: false,
            }],
        });
        let env = roundtrip(7, body.clone());
        assert_eq!(env.seq, 7);
        assert_eq!(env.body, Some(body));
    }

    #[test]
    fn login_failed_roundtrip() {
        let body = Body::LoginFailed(LoginFailed {
            reason_code: 1,
            message: "invalid credentials".into(),
        });
        let env = roundtrip(99, body.clone());
        assert_eq!(env.seq, 99);
        assert_eq!(env.body, Some(body));
    }
}
