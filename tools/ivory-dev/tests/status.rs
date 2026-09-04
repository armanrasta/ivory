//! CLI: `status --chain public` requires `IVORY_PUBLIC_RPC`.

use std::process::Command;

#[test]
fn status_public_errors_without_env() {
    let dir = tempfile::tempdir().unwrap();
    let app = dir.path().join("app");
    ivory_dev::new_project(&app).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_ivory-dev"))
        .args(["status", "--chain", "public"])
        .current_dir(&app)
        .env_remove("IVORY_PUBLIC_RPC")
        .output()
        .expect("run ivory-dev");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("IVORY_PUBLIC_RPC"),
        "stderr={err} stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
}
