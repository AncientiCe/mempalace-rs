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
        install_rule: true,
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

fn without_rule(mut options: InstallOptions) -> InstallOptions {
    options.install_rule = false;
    options
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
fn install_writes_cursor_rule_mdc() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());

    install_clients(&options(temp.path(), &binary_path, vec![Client::Cursor])).unwrap();

    let rule = fs::read_to_string(temp.path().join(".cursor/rules/mempalace.mdc")).unwrap();
    assert!(rule.contains("alwaysApply: true"));
    assert!(rule.contains("mempalace_status"));
    assert!(rule.contains("mempalace_search"));
}

#[test]
fn install_inserts_managed_block_into_existing_codex_agents_md() {
    let temp = TempDir::new().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("AGENTS.md"),
        "# Existing guidance\n\nKeep this line.\n",
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    install_clients(&options(temp.path(), &binary_path, vec![Client::Codex])).unwrap();

    let rule = fs::read_to_string(codex_dir.join("AGENTS.md")).unwrap();
    assert!(rule.contains("# Existing guidance"));
    assert!(rule.contains("Keep this line."));
    assert!(rule.contains("<!-- BEGIN MEMPALACE -->"));
    assert!(rule.contains("mempalace_kg_query"));
    assert!(rule.contains("<!-- END MEMPALACE -->"));
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
fn install_replaces_existing_managed_block_idempotently() {
    let temp = TempDir::new().unwrap();
    let claude_dir = temp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("CLAUDE.md"),
        "before\n\n<!-- BEGIN MEMPALACE -->\nstale\n<!-- END MEMPALACE -->\n\nafter\n",
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    let install_options = options(temp.path(), &binary_path, vec![Client::Claude]);
    let first = install_clients(&install_options).unwrap();
    let second = install_clients(&install_options).unwrap();

    assert!(first
        .rule_changed
        .iter()
        .any(|path| path.ends_with("CLAUDE.md")));
    assert!(second.rule_changed.is_empty());
    let rule = fs::read_to_string(claude_dir.join("CLAUDE.md")).unwrap();
    assert!(rule.contains("before"));
    assert!(rule.contains("after"));
    assert!(!rule.contains("stale"));
    assert_eq!(rule.matches("<!-- BEGIN MEMPALACE -->").count(), 1);
    assert!(claude_dir.join("CLAUDE.md.bak").exists());
    assert!(!claude_dir.join("CLAUDE.md.bak.bak").exists());
}

#[test]
fn install_with_no_rule_skips_rule_files() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let install_options = without_rule(options(temp.path(), &binary_path, vec![Client::Cursor]));

    let report = install_clients(&install_options).unwrap();

    assert_eq!(report.changed.len(), 1);
    assert!(report.rule_changed.is_empty());
    assert!(temp.path().join(".cursor/mcp.json").exists());
    assert!(!temp.path().join(".cursor/rules/mempalace.mdc").exists());
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
fn uninstall_removes_managed_block_only() {
    let temp = TempDir::new().unwrap();
    let codex_dir = temp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();
    fs::write(
        codex_dir.join("AGENTS.md"),
        "before\n\n<!-- BEGIN MEMPALACE -->\nmanaged\n<!-- END MEMPALACE -->\n\nafter\n",
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    uninstall_clients(&options(temp.path(), &binary_path, vec![Client::Codex])).unwrap();

    let rule = fs::read_to_string(codex_dir.join("AGENTS.md")).unwrap();
    assert!(rule.contains("before"));
    assert!(rule.contains("after"));
    assert!(!rule.contains("managed"));
    assert!(!rule.contains("<!-- BEGIN MEMPALACE -->"));
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn install_all_skips_claude_desktop_on_unsupported_platform() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    // Must not error on platforms where Claude Desktop is not supported
    let report = install_clients(&without_rule(options(
        temp.path(),
        &binary_path,
        vec![Client::All],
    )))
    .unwrap();
    let paths: Vec<_> = report
        .changed
        .iter()
        .chain(report.unchanged.iter())
        .collect();
    assert!(
        paths
            .iter()
            .all(|p| !p.to_string_lossy().contains("claude_desktop")),
        "claude_desktop_config.json should not appear on this platform"
    );
}

#[test]
fn install_claude_writes_to_dot_claude_json_not_subdirectory() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let report =
        install_clients(&options(temp.path(), &binary_path, vec![Client::Claude])).unwrap();

    assert_eq!(report.changed.len(), 1);
    // Must be ~/.claude.json (top-level), not ~/.claude/mcp_servers.json
    assert!(report.changed[0].ends_with(".claude.json"));
    assert!(!report.changed[0]
        .to_string_lossy()
        .contains("mcp_servers.json"));

    let config = read_json(&temp.path().join(".claude.json"));
    let server = &config["mcpServers"]["mempalace"];
    assert_eq!(
        server["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
    assert_eq!(server["args"], serde_json::json!(["mcp"]));
}

#[test]
fn install_claude_inserts_rule_block_into_claude_md() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    install_clients(&options(temp.path(), &binary_path, vec![Client::Claude])).unwrap();

    let rule = fs::read_to_string(temp.path().join(".claude/CLAUDE.md")).unwrap();
    assert!(rule.contains("<!-- BEGIN MEMPALACE -->"));
    assert!(rule.contains("mempalace_status"));
    assert!(rule.contains("<!-- END MEMPALACE -->"));
}

#[test]
fn uninstall_claude_removes_entry_from_dot_claude_json() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let install_options = options(temp.path(), &binary_path, vec![Client::Claude]);
    install_clients(&install_options).unwrap();
    let report = uninstall_clients(&without_rule(install_options)).unwrap();

    assert_eq!(report.changed.len(), 1);
    let config = read_json(&temp.path().join(".claude.json"));
    assert!(config["mcpServers"]["mempalace"].is_null());
}

#[cfg(target_os = "macos")]
#[test]
fn install_claude_desktop_writes_to_library_application_support() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let report = install_clients(&options(
        temp.path(),
        &binary_path,
        vec![Client::ClaudeDesktop],
    ))
    .unwrap();

    assert_eq!(report.changed.len(), 1);
    let expected = temp
        .path()
        .join("Library/Application Support/Claude/claude_desktop_config.json");
    assert_eq!(report.changed[0], expected);

    let config = read_json(&expected);
    let server = &config["mcpServers"]["mempalace"];
    assert_eq!(
        server["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
    assert_eq!(server["args"], serde_json::json!(["mcp"]));
}

#[cfg(target_os = "macos")]
#[test]
fn install_claude_desktop_preserves_existing_keys() {
    let temp = TempDir::new().unwrap();
    let desktop_dir = temp.path().join("Library/Application Support/Claude");
    fs::create_dir_all(&desktop_dir).unwrap();
    fs::write(
        desktop_dir.join("claude_desktop_config.json"),
        r#"{"preferences":{"theme":"dark"}}"#,
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    install_clients(&options(
        temp.path(),
        &binary_path,
        vec![Client::ClaudeDesktop],
    ))
    .unwrap();

    let config = read_json(&desktop_dir.join("claude_desktop_config.json"));
    assert_eq!(config["preferences"]["theme"], "dark");
    assert_eq!(
        config["mcpServers"]["mempalace"]["command"].as_str(),
        Some(binary_path.to_string_lossy().as_ref())
    );
}

#[cfg(target_os = "macos")]
#[test]
fn uninstall_claude_desktop_removes_only_mempalace() {
    let temp = TempDir::new().unwrap();
    let desktop_dir = temp.path().join("Library/Application Support/Claude");
    fs::create_dir_all(&desktop_dir).unwrap();
    fs::write(
        desktop_dir.join("claude_desktop_config.json"),
        r#"{"mcpServers":{"mempalace":{"command":"mp","args":["mcp"]},"other":{"command":"other"}}}"#,
    )
    .unwrap();

    let binary_path = fake_binary(temp.path());
    let install_options = without_rule(options(
        temp.path(),
        &binary_path,
        vec![Client::ClaudeDesktop],
    ));
    uninstall_clients(&install_options).unwrap();

    let config = read_json(&desktop_dir.join("claude_desktop_config.json"));
    assert!(config["mcpServers"]["mempalace"].is_null());
    assert_eq!(config["mcpServers"]["other"]["command"], "other");
}

#[cfg(target_os = "macos")]
#[test]
fn doctor_reports_claude_and_claude_desktop_paths() {
    let temp = TempDir::new().unwrap();
    let binary_path = fake_binary(temp.path());
    let install_options = options(
        temp.path(),
        &binary_path,
        vec![Client::Claude, Client::ClaudeDesktop],
    );

    let before = doctor(&install_options).unwrap();
    assert!(before
        .clients
        .iter()
        .any(|s| s.client == Client::Claude && !s.configured && s.path.ends_with(".claude.json")));
    assert!(before.clients.iter().any(|s| {
        s.client == Client::ClaudeDesktop
            && !s.configured
            && s.path.ends_with("claude_desktop_config.json")
    }));

    install_clients(&install_options).unwrap();
    let after = doctor(&install_options).unwrap();
    assert!(after
        .clients
        .iter()
        .all(|s| s.configured && s.points_to_expected_binary));
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
        status.client == Client::Cursor
            && status.configured
            && status.points_to_expected_binary
            && status.rule_installed
            && status.rule_path.ends_with(".cursor/rules/mempalace.mdc")
    }));
}
