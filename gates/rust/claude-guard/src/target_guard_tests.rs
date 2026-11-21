//! Which target directories count as nested, and how they are read from a
//! command.

use std::path::{Path, PathBuf};

use super::{nested_in, reason, resolve, target_dirs};

#[test]
fn target_dirs_are_read_from_the_environment_and_both_flag_forms() {
    assert_eq!(
        target_dirs("CARGO_TARGET_DIR=target/p2 cargo build"),
        vec!["target/p2"]
    );
    assert_eq!(
        target_dirs("cargo test --target-dir /x/t -p a"),
        vec!["/x/t"]
    );
    assert_eq!(
        target_dirs(r#"cargo build --target-dir="target/dx-watch""#),
        vec!["target/dx-watch"]
    );
    assert_eq!(
        target_dirs("export CARGO_TARGET_DIR='/tmp/t'"),
        vec!["/tmp/t"]
    );
}

#[test]
fn commands_without_a_target_dir_name_none() {
    for command in [
        "cargo build",
        "ls target",
        "echo --target-dir",
        "git status",
    ] {
        assert!(target_dirs(command).is_empty(), "{command}");
    }
}

#[test]
fn relative_dirs_resolve_against_the_working_directory() {
    let cwd = Path::new("/repo/crates/a");
    assert_eq!(
        resolve(cwd, "../../target/x"),
        PathBuf::from("/repo/target/x")
    );
    assert_eq!(resolve(cwd, "./t"), PathBuf::from("/repo/crates/a/t"));
    assert_eq!(resolve(cwd, "/abs/t"), PathBuf::from("/abs/t"));
}

#[test]
fn a_dir_under_a_build_directory_is_nested() {
    let build = |p: &Path| p == Path::new("/repo/target");
    assert_eq!(
        nested_in(Path::new("/repo/target/p2-screens"), build),
        Some(PathBuf::from("/repo/target"))
    );
    assert_eq!(
        nested_in(Path::new("/repo/target/a/b"), build),
        Some(PathBuf::from("/repo/target"))
    );
}

#[test]
fn the_build_directory_itself_and_siblings_are_not_nested() {
    let build = |p: &Path| p == Path::new("/repo/target");
    assert_eq!(nested_in(Path::new("/repo/target"), build), None);
    assert_eq!(nested_in(Path::new("/repo/target-wasm"), build), None);
    assert_eq!(nested_in(Path::new("/tmp/t"), build), None);
}

#[test]
fn a_real_cachedir_tag_marks_the_outer_directory() {
    let root = std::env::temp_dir().join(format!("target-guard-{}", std::process::id()));
    let outer = root.join("target");
    std::fs::create_dir_all(&outer).unwrap();
    std::fs::write(
        outer.join("CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55",
    )
    .unwrap();
    let found = nested_in(&outer.join("inner"), |p| p.join("CACHEDIR.TAG").is_file());
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(found, Some(outer));
}

#[test]
fn the_reason_names_both_directories() {
    let r = reason(Path::new("/repo/target/p2"), Path::new("/repo/target"));
    assert!(r.contains("/repo/target/p2") && r.contains("build directory /repo/target"));
}
