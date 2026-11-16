//! The editor's MCP surface: agents call tools that run inside the Bevy world
//! as one undoable step each. [`McpPlugin`] serves them; [`dispatch_plugin`]
//! is the Bevy side alone, fed by whoever inserts [`dispatch::AgentInbox`].

pub mod call;
pub mod coords;
pub mod dispatch;
pub mod handlers;
pub mod server;

use bevy::prelude::*;
use dispatch::AgentInbox;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::unbounded_channel;

/// The port `--mcp` listens on unless `--mcp-port` says otherwise.
pub const DEFAULT_PORT: u16 = 7700;

/// The MCP port from the command line: `None` without `--mcp`, otherwise
/// `--mcp-port <n>` or [`DEFAULT_PORT`]. Other arguments are ignored.
pub fn parse_mcp_args(args: impl IntoIterator<Item = String>) -> Result<Option<u16>, String> {
    let args: Vec<String> = args.into_iter().collect();
    let enabled = args.iter().any(|arg| arg == "--mcp");
    let port = match args.iter().position(|arg| arg == "--mcp-port") {
        None => None,
        Some(at) => {
            let value = args.get(at + 1).ok_or("--mcp-port needs a port number")?;
            let port = value
                .parse::<u16>()
                .map_err(|error| format!("--mcp-port '{value}': {error}"))?;
            Some(port)
        }
    };
    match (enabled, port) {
        (false, None) => Ok(None),
        (false, Some(_)) => Err("--mcp-port needs --mcp".into()),
        (true, port) => Ok(Some(port.unwrap_or(DEFAULT_PORT))),
    }
}

/// Serves the editor over MCP on `127.0.0.1:<port>/mcp` from a background
/// thread. A server that cannot start exits the editor with an error.
pub struct McpPlugin {
    pub port: u16,
}

/// Why the server thread stopped, once it has.
#[derive(Resource, Clone, Default)]
struct McpServerError(Arc<Mutex<Option<String>>>);

impl Plugin for McpPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = unbounded_channel();
        let failure = McpServerError::default();
        let slot = failure.0.clone();
        let port = self.port;
        std::thread::Builder::new()
            .name("mcp".into())
            .spawn(move || {
                let outcome = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(anyhow::Error::from)
                    .and_then(|runtime| runtime.block_on(server::serve(port, tx)));
                if let (Err(error), Ok(mut slot)) = (outcome, slot.lock()) {
                    *slot = Some(format!("{error:#}"));
                }
            })
            .expect("spawning the mcp thread");
        app.insert_resource(AgentInbox(rx))
            .insert_resource(failure)
            .add_systems(Update, exit_on_server_error);
        dispatch_plugin(app);
    }
}

fn exit_on_server_error(failure: Res<McpServerError>, mut exit: MessageWriter<AppExit>) {
    let Some(error) = failure.0.lock().ok().and_then(|mut slot| slot.take()) else {
        return;
    };
    error!("MCP server stopped: {error}");
    exit.write(AppExit::error());
}

/// Runs agent calls from [`dispatch::AgentInbox`] every frame, before the
/// editor acts on the open requests they queue.
pub fn dispatch_plugin(app: &mut App) {
    app.add_systems(
        Update,
        dispatch::drain_calls.before(crate::map::open::StartOpening),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<u16>, String> {
        parse_mcp_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn mcp_flags_parse() {
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(parse(&["--mcp"]), Ok(Some(DEFAULT_PORT)));
        assert_eq!(parse(&["--mcp", "--mcp-port", "9000"]), Ok(Some(9000)));
        assert!(parse(&["--mcp", "--mcp-port", "abc"]).is_err());
        assert!(parse(&["--mcp", "--mcp-port"]).is_err());
        assert!(parse(&["--mcp-port", "9000"]).is_err());
    }
}
