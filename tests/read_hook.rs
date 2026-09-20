/// Node tests for the cross-host PreToolUse hook script.
use std::process::Command;

#[test]
fn read_hook_cross_host_payloads() {
    let status = Command::new("node")
        .args(["--test", "registry/scripts/read-hook.test.mjs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("node is required to run the PreToolUse hook tests");

    assert!(
        status.success(),
        "node --test registry/scripts/read-hook.test.mjs failed: {status:?}"
    );
}
