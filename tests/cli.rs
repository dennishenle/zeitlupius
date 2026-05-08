use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn cmd(td: &TempDir) -> Command {
    let mut c = Command::cargo_bin("zeitlupius").unwrap();
    c.arg("--data-dir").arg(td.path());
    c
}

#[test]
fn create_list_delete_cycle() {
    let td = TempDir::new().unwrap();

    cmd(&td).args(["create", "rust-zlp"]).assert().success();
    cmd(&td)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("rust-zlp"));
    cmd(&td)
        .args(["delete", "rust-zlp", "--force"])
        .assert()
        .success();
    cmd(&td)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn start_then_status() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td).args(["start", "p"]).assert().success();
    cmd(&td)
        .args(["status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("p"));
}

#[test]
fn missing_project_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["start", "nope"]).assert().code(1);
}

#[test]
fn invalid_date_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td)
        .args(["report", "--from", "31.02.2026", "--to", "01.03.2026"])
        .assert()
        .code(1);
}

#[test]
fn report_week_runs() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td)
        .args(["report", "--week"])
        .assert()
        .success()
        .stdout(predicate::str::contains("TOTAL"));
}
