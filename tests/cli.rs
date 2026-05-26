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

#[test]
fn session_list_shows_id_after_start_stop() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td).args(["start", "p"]).assert().success();
    cmd(&td).args(["stop", "p"]).assert().success();
    cmd(&td)
        .args(["session", "list", "p"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"));
}

#[test]
fn session_delete_force_removes_one() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td).args(["start", "p"]).assert().success();
    cmd(&td).args(["stop", "p"]).assert().success();

    let json = cmd(&td)
        .args(["session", "list", "p", "--json"])
        .output()
        .unwrap()
        .stdout;
    let json = String::from_utf8(json).unwrap();
    let id_start = json.find("\"id\":\"").unwrap() + "\"id\":\"".len();
    let id_end = id_start + 8;
    let id = &json[id_start..id_end];

    cmd(&td)
        .args(["session", "delete", "p", id, "--force"])
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted"));

    cmd(&td)
        .args(["session", "list", "p", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[]"));
}

#[test]
fn session_delete_bad_id_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td)
        .args(["session", "delete", "p", "notvalid", "--force"])
        .assert()
        .code(1);
}

#[test]
fn session_delete_unknown_id_user_error() {
    let td = TempDir::new().unwrap();
    cmd(&td).args(["create", "p"]).assert().success();
    cmd(&td)
        .args(["session", "delete", "p", "aaaaaaaa", "--force"])
        .assert()
        .code(1);
}

#[test]
fn legacy_csv_is_migrated_on_session_list() {
    let td = TempDir::new().unwrap();
    let proj_dir = td.path().join("projects");
    std::fs::create_dir_all(&proj_dir).unwrap();
    let legacy_path = proj_dir.join("p.csv");
    std::fs::write(
        &legacy_path,
        "start,stop,note\n2026-05-04T09:00:00+00:00[UTC],2026-05-04T10:00:00+00:00[UTC],morning\n",
    )
    .unwrap();

    cmd(&td)
        .args(["session", "list", "p", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\":\""));

    let after = std::fs::read_to_string(&legacy_path).unwrap();
    assert!(
        after.contains("start,stop,note,id"),
        "header not rewritten: {after}"
    );
}
