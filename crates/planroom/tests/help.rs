use std::process::Command;

fn planroom(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_planroom")).args(args).output().expect("the binary runs")
}

#[test]
fn help_prints_usage_and_exits_zero() {
    let out = planroom(&["--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Usage: planroom"), "{stdout}");
}

#[test]
fn no_arguments_prints_the_help() {
    let out = planroom(&[]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("Usage: planroom"));
}

#[test]
fn version_names_the_workspace_version() {
    let out = planroom(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        format!("planroom {}", env!("CARGO_PKG_VERSION"))
    );
}
