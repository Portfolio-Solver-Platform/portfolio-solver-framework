use assert_cmd::Command;

use predicates::prelude::predicate::str::contains; // Used for string matching
#[test]
fn test_cli_success() {
    let path = assert_cmd::cargo::cargo_bin!("portfolio-solver-framework");
    let mut cmd = Command::new(path);
    cmd.args([
        "tests/data/accap.mzn",
        "tests/data/accap_instance6.dzn",
        "--debug-verbosity",
        "warning",
    ])
    .assert()
    .success()
    .stdout(contains("=========="));
}

// #[test]
// fn test_cli_failure() {
//     let mut cmd = cargo::cargo_bin!("my_project_name").unwrap();

//     // Run without arguments
//     cmd.assert()
//         .failure() // Check that exit code is NOT 0
//         .stderr(contains("Error: Missing name argument"));
// }
