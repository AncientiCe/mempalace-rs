use std::fs;
use std::path::{Path, PathBuf};

use mempalace::install::{
    doctor, install_clients, uninstall_clients, Client, InstallOptions, Scope,
};
use serde_json::Value;
use tempfile::TempDir;
use toml_edit::DocumentMut;

fn options(home: &Path, binary_path: &Path, clients: Vec<Client>) -> InstallOptions {
    InstallOptions {
        clients,
        scope: Scope::User,
        project_dir: None,
        home_dir: home.to_path_buf(),
        binary_path: binary_path.to_path_buf(),
        dry_run: false,
        force: false,
    }
}

fn fake_binary(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("bin").join("mempalace.exe")
    } else {
        home.join("bin").join("mempalace")
    }
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn install_writes_cursor_config_to_temp_home() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let report =
        install_clients(&options(temp.path(), &binary_path, vec![Client::Cursor])).unwrap();

    assert_eq!(report.changed.len(), 1);
    let config = read_json(&temp.path().join(".cursor").join("mcp.json"));
    let server = &config["mcpServers"]["mempalace"];
    assert_eq!(
        server["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
    assert_eq!(server["args"], serde_json::json!(["mcp"]));
}

#[test]
fn install_merges_into_existing_cursor_config() {
    let temp = TempDir::new().unwrap();
    let cursor_dir = temp.path().join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    fs::write(
        cursor_dir.join("mcp.json"),
        r#"{"mcpServers":{"other":{"command":"other-tool","args":["serve"]}}}"#,
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    install_clients(&options(temp.path(), &binary_path, vec![Client::Cursor])).unwrap();

    let config = read_json(&cursor_dir.join("mcp.json"));
    assert_eq!(config["mcpServers"]["other"]["command"], "other-tool");
    assert_eq!(
        config["mcpServers"]["mempalace"]["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
    assert!(cursor_dir.join("mcp.json.bak").exists());
}

#[test]
fn install_merges_codex_toml_preserving_comments() {
    let temp = TempDir::new().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("config.toml"),
        "# keep this comment\nmodel = \"gpt-5\"\n",
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    install_clients(&options(temp.path(), &binary_path, vec![Client::Codex])).unwrap();

    let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
    assert!(config.contains("# keep this comment"));
    assert!(config.contains("model = \"gpt-5\""));
    assert!(config.contains("[mcp_servers.mempalace]"));
    let parsed = config.parse::<DocumentMut>().unwrap();
    assert_eq!(
        parsed["mcp_servers"]["mempalace"]["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
    assert_eq!(
        parsed["mcp_servers"]["mempalace"]["args"][0].as_str(),
        Some("mcp")
    );
}

#[test]
fn install_is_idempotent() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let install_options = options(temp.path(), &binary_path, vec![Client::Cursor]);

    let first = install_clients(&install_options).unwrap();
    let second = install_clients(&install_options).unwrap();

    assert_eq!(first.changed.len(), 1);
    assert!(second.changed.is_empty());
    let config = read_json(&temp.path().join(".cursor").join("mcp.json"));
    assert_eq!(
        config["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|key| key.as_str() == "mempalace")
            .count(),
        1
    );
    assert!(!temp.path().join(".cursor").join("mcp.json.bak").exists());
}

#[test]
fn uninstall_removes_only_mempalace_entry() {
    let temp = TempDir::new().unwrap();
    let cursor_dir = temp.path().join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    fs::write(
        cursor_dir.join("mcp.json"),
        r#"{"mcpServers":{"mempalace":{"command":"mempalace","args":["mcp"]},"other":{"command":"other-tool"}}}"#,
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    uninstall_clients(&options(temp.path(), &binary_path, vec![Client::Cursor])).unwrap();

    let config = read_json(&cursor_dir.join("mcp.json"));
    assert!(config["mcpServers"]["mempalace"].is_null());
    assert_eq!(config["mcpServers"]["other"]["command"], "other-tool");
}

#[test]
fn doctor_reports_status_correctly() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let install_options = options(temp.path(), &binary_path, vec![Client::Cursor]);

    let before = doctor(&install_options).unwrap();
    assert!(before.clients.iter().any(|status| {
        status.client == Client::Cursor
            && !status.configured
            && status.path.ends_with(".cursor/mcp.json")
    }));

    install_clients(&install_options).unwrap();
    let after = doctor(&install_options).unwrap();
    assert!(after.clients.iter().any(|status| {
        status.client == Client::Cursor && status.configured && status.points_to_expected_binary
    }));
}
