use assert_cmd::Command;

use predicates::prelude::predicate::str::contains;
#[test]
fn test_cli_success() {
    // let path = assert_cmd::cargo::cargo_bin!("portfolio-solver-framework");
    let mut cmd = Command::new("minizinc");
    cmd.args([
        "--solver",
        "sunny",
        "tests/data/accap.mzn",
        "tests/data/accap_instance6.dzn",
    ])
    .assert()
    .success()
    .stdout(contains("=========="));
}

#[test]
fn test_cli_failure() {
    // let path = assert_cmd::cargo::cargo_bin!("portfolio-solver-framework");
    let mut cmd = Command::new("minizinc");
    cmd.args(["--solver", "sunny", "tests/data/accap_instance6.dzn"])
        .assert()
        .failure()
        .stderr(contains("ERROR"));
}
