use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_monorepo_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "ag_test_federation_{}_{}_{}",
        name,
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()
    ));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_auto_discover_cargo_workspaces() {
    let root = temp_monorepo_dir("cargo_ws");
    let cargo_toml = root.join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"[workspace]
members = [
    "crates/core",
    "crates/cli",
]
"#,
    ).unwrap();

    let core_dir = root.join("crates/core");
    let cli_dir = root.join("crates/cli");
    fs::create_dir_all(&core_dir).unwrap();
    fs::create_dir_all(&cli_dir).unwrap();

    let discovered = auto_discover_workspaces(&root);
    assert_eq!(discovered.len(), 2);
    let names: Vec<String> = discovered.into_iter().map(|p| p.name).collect();
    assert!(names.contains(&"core".to_string()));
    assert!(names.contains(&"cli".to_string()));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_auto_discover_package_json_workspaces() {
    let root = temp_monorepo_dir("npm_ws");
    let pkg_json = root.join("package.json");
    fs::write(
        &pkg_json,
        r#"{
  "name": "root",
  "workspaces": [
    "packages/ui",
    "packages/api"
  ]
}"#,
    ).unwrap();

    let ui_dir = root.join("packages/ui");
    let api_dir = root.join("packages/api");
    fs::create_dir_all(&ui_dir).unwrap();
    fs::create_dir_all(&api_dir).unwrap();

    let discovered = auto_discover_workspaces(&root);
    assert_eq!(discovered.len(), 2);
    let names: Vec<String> = discovered.into_iter().map(|p| p.name).collect();
    assert!(names.contains(&"ui".to_string()));
    assert!(names.contains(&"api".to_string()));

    let _ = fs::remove_dir_all(&root);
}
