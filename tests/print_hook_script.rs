/// Integration test for `transpile --print-hook-script`
use std::process::Command;

fn transpile_bin() -> Command {
    // Use `cargo run --bin transpile` so the test works before installation.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_transpile"));
    cmd
}

#[test]
fn print_hook_script_exits_zero() {
    let output = transpile_bin()
        .arg("--print-hook-script")
        .output()
        .expect("failed to run transpile");

    assert!(
        output.status.success(),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn print_hook_script_stdout_contains_shebang() {
    let output = transpile_bin()
        .arg("--print-hook-script")
        .output()
        .expect("failed to run transpile");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("#!/usr/bin/env bash"),
        "expected shebang in output, got:\n{stdout}"
    );
}

#[test]
fn print_hook_script_stdout_contains_transpile_invocation() {
    let output = transpile_bin()
        .arg("--print-hook-script")
        .output()
        .expect("failed to run transpile");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("transpile --input"),
        "expected transpile invocation in output, got:\n{stdout}"
    );
}

#[test]
fn print_hook_script_stderr_is_empty() {
    let output = transpile_bin()
        .arg("--print-hook-script")
        .output()
        .expect("failed to run transpile");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "expected empty stderr, got:\n{stderr}"
    );
}
