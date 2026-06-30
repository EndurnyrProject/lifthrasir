//! Boundary guard: `game-engine` must depend only on `net-contract` and never on a
//! network transport/codec. Adapters (e.g. `net-aesir`, the aesir QUIC adapter) are
//! wired in the `lifthrasir` binary. See
//! `specs/2026-06-30-network-decoupling/design.md` §"Enforcement".

use std::process::Command;

const FORBIDDEN: &[&str] = &["bevy_quinnet", "prost", "net-aesir"];

#[test]
fn game_engine_has_no_transport_or_codec_dependency() {
    let output = Command::new(env!("CARGO"))
        .args(["tree", "-p", "game-engine", "-e", "normal"])
        .output()
        .expect("failed to spawn `cargo tree`; cannot verify the network boundary");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "`cargo tree -p game-engine -e normal` failed (status {:?}); cannot verify the \
         network boundary.\nstderr:\n{stderr}",
        output.status.code()
    );

    let offenders: Vec<&str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|crate_name| stdout.contains(crate_name))
        .collect();

    assert!(
        offenders.is_empty(),
        "game-engine regained a forbidden network dependency: {offenders:?}.\n\
         game-engine must depend only on net-contract (the protocol-neutral Bevy Message \
         contract); transport/codec adapters are wired in the binary (lifthrasir/src/main.rs), \
         not in game-engine — see specs/2026-06-30-network-decoupling/design.md.\n\
         Full `cargo tree` output:\n{stdout}"
    );
}
