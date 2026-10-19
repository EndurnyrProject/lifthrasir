use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::proto::aesir::net::{CharServerInfo, LoginFailed, LoginResponse};
use net_contract::dto::{ServerInfo, ServerType};
use net_contract::events::{LoginAccepted, LoginRefused};

fn char_server_to_server_info(cs: CharServerInfo) -> Result<ServerInfo, String> {
    let ip = Ipv4Addr::from_str(&cs.ip)
        .map(u32::from)
        .map_err(|_| format!("server '{}' has an invalid address '{}'", cs.name, cs.ip))?;
    Ok(ServerInfo {
        ip,
        port: cs.port as u16,
        name: cs.name,
        users: cs.user_count as u16,
        server_type: ServerType::from(cs.server_type as u16),
        new_server: cs.is_new as u16,
    })
}

pub fn login_response_to_accepted(
    resp: LoginResponse,
    username: String,
) -> Result<LoginAccepted, String> {
    let server_list = resp
        .char_servers
        .into_iter()
        .map(char_server_to_server_info)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LoginAccepted {
        account_id: resp.account_id,
        login_id1: resp.login_id1,
        login_id2: resp.login_id2,
        sex: resp.sex as u8,
        server_list,
        username,
        auth_token: resp.auth_token,
    })
}

pub fn login_failed_to_refused(failed: LoginFailed, username: String) -> LoginRefused {
    LoginRefused {
        username,
        error_code: failed.reason_code as u8,
        error_message: failed.message,
        block_date: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_response_maps_to_accepted() {
        let resp = LoginResponse {
            account_id: 2000001,
            login_id1: 111,
            login_id2: 222,
            sex: 1,
            auth_token: "deadbeefcafebabe".into(),
            char_servers: vec![
                CharServerInfo {
                    name: "Midgard".into(),
                    ip: "127.0.0.1".into(),
                    port: 6121,
                    user_count: 5,
                    server_type: 0,
                    is_new: false,
                },
                CharServerInfo {
                    name: "Valhalla".into(),
                    ip: "10.0.0.2".into(),
                    port: 6122,
                    user_count: 42,
                    server_type: 1,
                    is_new: true,
                },
            ],
        };

        let accepted =
            login_response_to_accepted(resp, "player".into()).expect("valid server list");

        assert_eq!(accepted.account_id, 2000001);
        assert_eq!(accepted.login_id1, 111);
        assert_eq!(accepted.login_id2, 222);
        assert_eq!(accepted.sex, 1);
        assert_eq!(accepted.username, "player");
        assert_eq!(accepted.auth_token, "deadbeefcafebabe");
        assert_eq!(accepted.server_list.len(), 2);

        let first = &accepted.server_list[0];
        assert_eq!(first.name, "Midgard");
        assert_eq!(first.ip, u32::from(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(first.ip_string(), "127.0.0.1");
        assert_eq!(first.port, 6121);
        assert_eq!(first.users, 5);
        assert_eq!(first.server_type, ServerType::Normal);
        assert_eq!(first.new_server, 0);

        let second = &accepted.server_list[1];
        assert_eq!(second.ip_string(), "10.0.0.2");
        assert_eq!(second.port, 6122);
        assert_eq!(second.server_type, ServerType::Maintenance);
        assert_eq!(second.new_server, 1);
    }

    #[test]
    fn login_response_malformed_ip_is_rejected() {
        let resp = LoginResponse {
            account_id: 2000001,
            login_id1: 111,
            login_id2: 222,
            sex: 1,
            auth_token: "deadbeefcafebabe".into(),
            char_servers: vec![CharServerInfo {
                name: "Midgard".into(),
                ip: "not-an-ip".into(),
                port: 6121,
                user_count: 5,
                server_type: 0,
                is_new: false,
            }],
        };

        let err = login_response_to_accepted(resp, "player".into())
            .expect_err("malformed ip should be rejected");
        assert!(err.contains("Midgard"));
        assert!(err.contains("not-an-ip"));
    }

    #[test]
    fn login_failed_maps_to_refused() {
        let failed = LoginFailed {
            reason_code: 1,
            message: "invalid credentials".into(),
        };

        let refused = login_failed_to_refused(failed, "player".into());

        assert_eq!(refused.username, "player");
        assert_eq!(refused.error_code, 1);
        assert_eq!(refused.error_message, "invalid credentials");
        assert_eq!(refused.block_date, None);
    }
}
