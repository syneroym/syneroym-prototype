use predicates::prelude::*;

#[test]
fn test_cli_subcommand_version() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("syneroym-cli");
    cmd.arg("version")
        .assert()
        .success()
        .stdout(predicate::str::contains("Version:"));
}

#[test]
fn test_cli_flag_version() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("syneroym-cli");
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("syneroym-cli"));
}

#[test]
fn test_cli_help() {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("syneroym-cli");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:"));
}
