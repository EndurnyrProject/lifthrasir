use anyhow::Context;
use std::path::Path;

pub fn run(src_dir: &Path, out_file: &Path) -> anyhow::Result<()> {
    let fds = protox::compile(["aesir.proto"], [src_dir])
        .with_context(|| format!("compiling aesir.proto from {}", src_dir.display()))?;

    let tmp = tempfile::tempdir().context("creating temp dir for codegen")?;

    prost_build::Config::new()
        .out_dir(tmp.path())
        .compile_fds(fds)
        .context("generating Rust from FileDescriptorSet")?;

    if let Some(parent) = out_file.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating output dir {}", parent.display()))?;
    }

    std::fs::copy(tmp.path().join("aesir.net.rs"), out_file)
        .with_context(|| format!("copying generated file to {}", out_file.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const FIXTURE_PROTO: &str = r#"syntax = "proto3";
package aesir.net;

message Envelope {
  uint32 seq = 1;
  oneof body {
    Hello hello = 16;
    HelloAck hello_ack = 17;
    LoginRequest login_request = 18;
    LoginResponse login_response = 19;
    LoginFailed login_failed = 20;
  }
}
message Hello        { uint32 protocol_version = 1; string build = 2; }
message HelloAck     { uint32 protocol_version = 1; bool accepted = 2; }
message LoginRequest { string username = 1; string password = 2; uint32 client_version = 3; }
message CharServerInfo { string name = 1; string ip = 2; uint32 port = 3;
                         uint32 user_count = 4; uint32 server_type = 5; bool is_new = 6; }
message LoginResponse { uint32 account_id = 1; uint32 login_id1 = 2; uint32 login_id2 = 3;
                        uint32 sex = 4; string auth_token = 5;
                        repeated CharServerInfo char_servers = 6; }
message LoginFailed   { uint32 reason_code = 1; string message = 2; }
"#;

    #[test]
    fn gen_proto_emits_expected_structs() {
        let base = std::env::temp_dir().join(format!("proto_gen_test_{}", std::process::id()));
        let src_dir = base.join("src");
        let out_file = base.join("out").join("aesir.net.rs");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("aesir.proto"), FIXTURE_PROTO).unwrap();

        run(&src_dir, &out_file).expect("proto_gen::run failed");

        let generated = fs::read_to_string(&out_file).expect("output file missing");

        for expected in &[
            "struct Envelope",
            "struct Hello",
            "struct HelloAck",
            "struct LoginRequest",
            "struct LoginResponse",
            "struct CharServerInfo",
            "struct LoginFailed",
        ] {
            assert!(
                generated.contains(expected),
                "generated output missing: {expected}"
            );
        }

        fs::remove_dir_all(&base).ok();
    }
}
