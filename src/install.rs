use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, DocumentMut, Item, Table};

const RULE_BEGIN: &str = "<!-- BEGIN MEMPALACE -->";
const RULE_END: &str = "<!-- END MEMPALACE -->";
const RULE_BODY: &str = "**MANDATORY — follow every step, every session, no exceptions.**\n\n1. **SESSION START**: Call `mempalace_status` BEFORE doing anything else.\n2. **BEFORE ANSWERING** about any person, project, past decision, or preference: call `mempalace_search` or `mempalace_kg_query` first. Never answer from training data alone.\n3. **AFTER SUBSTANTIVE WORK**: call `mempalace_diary_write` to record what happened.\n4. **WHEN FACTS CHANGE**: call `mempalace_kg_invalidate` on the old fact, then `mempalace_kg_add` for the new one.\n\nSkipping any step is a protocol violation. Storage is not memory; this protocol is.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Client {
    Cursor,
    Codex,
    Claude,
    ClaudeDesktop,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    User,
    Project,
}

#[derive(Clone, Debug)]
pub struct InstallOptions {
    pub clients: Vec<Client>,
    pub scope: Scope,
    pub project_dir: Option<PathBuf>,
    pub home_dir: PathBuf,
    pub binary_path: PathBuf,
    pub dry_run: bool,
    pub force: bool,
    pub install_rule: bool,
}

#[derive(Clone, Debug, Default)]
pub struct InstallReport {
    pub changed: Vec<PathBuf>,
    pub unchanged: Vec<PathBuf>,
    pub rule_changed: Vec<PathBuf>,
    pub rule_unchanged: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientStatus {
    pub client: Client,
    pub path: PathBuf,
    pub configured: bool,
    pub points_to_expected_binary: bool,
    pub command: Option<String>,
    pub rule_path: PathBuf,
    pub rule_installed: bool,
}

#[derive(Clone, Debug)]
pub struct DoctorReport {
    pub binary_path: PathBuf,
    pub palace_db_path: PathBuf,
    pub drawer_count: Option<i64>,
    pub clients: Vec<ClientStatus>,
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Client::Cursor => f.write_str("cursor"),
            Client::Codex => f.write_str("codex"),
            Client::Claude => f.write_str("claude"),
            Client::ClaudeDesktop => f.write_str("claude-desktop"),
            Client::All => f.write_str("all"),
        }
    }
}

impl std::str::FromStr for Client {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "cursor" => Ok(Client::Cursor),
            "codex" => Ok(Client::Codex),
            "claude" | "claude-code" => Ok(Client::Claude),
            "claude-desktop" => Ok(Client::ClaudeDesktop),
            "all" => Ok(Client::All),
            other => Err(anyhow!("unknown MCP client: {other}")),
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scope::User => f.write_str("user"),
            Scope::Project => f.write_str("project"),
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "user" => Ok(Scope::User),
            "project" => Ok(Scope::Project),
            other => Err(anyhow!("unknown install scope: {other}")),
        }
    }
}

impl InstallOptions {
    pub fn for_current_process(
        clients: Vec<Client>,
        scope: Scope,
        project_dir: Option<PathBuf>,
    ) -> Result<Self> {
        Ok(Self {
            clients,
            scope,
            project_dir,
            home_dir: home_dir()?,
            binary_path: std::env::current_exe().context("failed to resolve current executable")?,
            dry_run: false,
            force: false,
            install_rule: true,
        })
    }
}

pub fn install_clients(options: &InstallOptions) -> Result<InstallReport> {
    let mut report = InstallReport::default();
    for client in expand_clients(&options.clients) {
        let path = config_path(options, client)?;
        let changed = match client {
            Client::Cursor | Client::Claude | Client::ClaudeDesktop => {
                write_json_client(&path, &options.binary_path, options.dry_run)?
            }
            Client::Codex => write_codex_client(&path, &options.binary_path, options.dry_run)?,
            Client::All => false,
        };
        if changed {
            report.changed.push(path);
        } else {
            report.unchanged.push(path);
        }
        if options.install_rule {
            let target = rule_target(options, client)?;
            let changed = install_rule(&target, options.dry_run)?;
            if changed {
                report.rule_changed.push(target.path);
            } else {
                report.rule_unchanged.push(target.path);
            }
        }
    }
    Ok(report)
}

pub fn uninstall_clients(options: &InstallOptions) -> Result<InstallReport> {
    let mut report = InstallReport::default();
    for client in expand_clients(&options.clients) {
        let path = config_path(options, client)?;
        let changed = match client {
            Client::Cursor | Client::Claude | Client::ClaudeDesktop => {
                remove_json_client(&path, options.dry_run)?
            }
            Client::Codex => remove_codex_client(&path, options.dry_run)?,
            Client::All => false,
        };
        if changed {
            report.changed.push(path);
        } else {
            report.unchanged.push(path);
        }
        if options.install_rule {
            let target = rule_target(options, client)?;
            let changed = uninstall_rule(&target, options.dry_run)?;
            if changed {
                report.rule_changed.push(target.path);
            } else {
                report.rule_unchanged.push(target.path);
            }
        }
    }
    Ok(report)
}

pub fn doctor(options: &InstallOptions) -> Result<DoctorReport> {
    let config = crate::config::MempalaceConfig::new();
    let palace_db_path = config.palace_db_path();
    let drawer_count = if palace_db_path.exists() {
        crate::db::open(&palace_db_path)
            .and_then(|conn| crate::store::count_drawers(&conn))
            .ok()
    } else {
        None
    };

    let mut clients = Vec::new();
    for client in expand_clients(&options.clients) {
        let path = config_path(options, client)?;
        let target = rule_target(options, client)?;
        let command = read_configured_command(client, &path)?;
        let expected = path_to_string(&options.binary_path);
        let rule_installed = rule_installed(&target)?;
        clients.push(ClientStatus {
            client,
            path,
            configured: command.is_some(),
            points_to_expected_binary: command.as_deref() == Some(expected.as_str()),
            command,
            rule_path: target.path,
            rule_installed,
        });
    }

    Ok(DoctorReport {
        binary_path: options.binary_path.clone(),
        palace_db_path,
        drawer_count,
        clients,
    })
}

pub fn print_install_report(action: &str, report: &InstallReport) {
    for path in &report.changed {
        println!("  {action}: {}", path.display());
    }
    for path in &report.unchanged {
        println!("  unchanged: {}", path.display());
    }
    for path in &report.rule_changed {
        println!("  rule {action}: {}", path.display());
    }
    for path in &report.rule_unchanged {
        println!("  rule unchanged: {}", path.display());
    }
}

pub fn print_doctor_report(report: &DoctorReport) {
    println!("MemPalace doctor");
    println!("  Binary: {}", report.binary_path.display());
    println!("  Palace DB: {}", report.palace_db_path.display());
    match report.drawer_count {
        Some(count) => println!("  Drawers: {count}"),
        None => println!("  Drawers: no palace database found yet"),
    }
    for status in &report.clients {
        let state = if status.points_to_expected_binary {
            "configured"
        } else if status.configured {
            "configured elsewhere"
        } else {
            "missing"
        };
        println!("  {}: {state} ({})", status.client, status.path.display());
        let rule_state = if status.rule_installed {
            "rule installed"
        } else {
            "rule missing"
        };
        println!("      {rule_state} ({})", status.rule_path.display());
    }
    if report.drawer_count.unwrap_or(0) == 0 {
        println!("  Next: mempalace init <project> && mempalace mine <project>");
    }
}

fn expand_clients(clients: &[Client]) -> Vec<Client> {
    if clients.is_empty() || clients.contains(&Client::All) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return vec![
            Client::Cursor,
            Client::Codex,
            Client::Claude,
            Client::ClaudeDesktop,
        ];
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        return vec![Client::Cursor, Client::Codex, Client::Claude];
    } else {
        clients
            .iter()
            .copied()
            .filter(|client| *client != Client::All)
            .collect()
    }
}

fn config_path(options: &InstallOptions, client: Client) -> Result<PathBuf> {
    match client {
        Client::Cursor => match options.scope {
            Scope::User => Ok(options.home_dir.join(".cursor").join("mcp.json")),
            Scope::Project => {
                let project_dir = options.project_dir.as_ref().ok_or_else(|| {
                    anyhow!("--path is required for project-scope Cursor installs")
                })?;
                Ok(project_dir.join(".cursor").join("mcp.json"))
            }
        },
        Client::Codex => Ok(options.home_dir.join(".codex").join("config.toml")),
        // Claude Code CLI reads from ~/.claude.json (top-level file, not ~/.claude/ directory)
        Client::Claude => Ok(options.home_dir.join(".claude.json")),
        Client::ClaudeDesktop => claude_desktop_config_path(&options.home_dir),
        Client::All => Err(anyhow!("all is not a concrete client")),
    }
}

fn claude_desktop_config_path(_home_dir: &Path) -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Ok(_home_dir.join("Library/Application Support/Claude/claude_desktop_config.json"))
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA").ok_or_else(|| anyhow!("APPDATA not set"))?;
        Ok(PathBuf::from(appdata)
            .join("Claude")
            .join("claude_desktop_config.json"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow!("claude-desktop is not supported on this platform"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleKind {
    Standalone,
    ManagedBlock,
}

#[derive(Clone, Debug)]
struct RuleTarget {
    path: PathBuf,
    kind: RuleKind,
}

fn rule_target(options: &InstallOptions, client: Client) -> Result<RuleTarget> {
    let project_dir = || {
        options
            .project_dir
            .as_ref()
            .ok_or_else(|| anyhow!("--path is required for project-scope rule installs"))
    };
    let path = match (client, options.scope) {
        (Client::Cursor, Scope::User) => options.home_dir.join(".cursor/rules/mempalace.mdc"),
        (Client::Cursor, Scope::Project) => project_dir()?.join(".cursor/rules/mempalace.mdc"),
        (Client::Codex, Scope::User) => options.home_dir.join(".codex/AGENTS.md"),
        (Client::Codex, Scope::Project) => project_dir()?.join("AGENTS.md"),
        (Client::Claude, Scope::User) => options.home_dir.join(".claude/CLAUDE.md"),
        (Client::Claude, Scope::Project) => project_dir()?.join("CLAUDE.md"),
        // Claude Desktop has no rules/prompts file to inject into
        (Client::ClaudeDesktop, _) => options.home_dir.join(".claude/CLAUDE.md"),
        (Client::All, _) => return Err(anyhow!("all is not a concrete client")),
    };
    let kind = match client {
        Client::Cursor => RuleKind::Standalone,
        Client::Codex | Client::Claude | Client::ClaudeDesktop => RuleKind::ManagedBlock,
        Client::All => return Err(anyhow!("all is not a concrete client")),
    };
    Ok(RuleTarget { path, kind })
}

fn home_dir() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(profile));
    }
    Err(anyhow!("could not determine home directory"))
}

fn write_json_client(path: &Path, binary_path: &Path, dry_run: bool) -> Result<bool> {
    let existing = read_json_config(path)?;
    let mut next = existing.clone();
    ensure_json_server(&mut next, binary_path)?;
    write_if_changed(path, existing, next, dry_run)
}

fn remove_json_client(path: &Path, dry_run: bool) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = read_json_config(path)?;
    let mut next = existing.clone();
    let Some(servers) = next.get_mut("mcpServers").and_then(Value::as_object_mut) else {
        return Ok(false);
    };
    if servers.remove("mempalace").is_none() {
        return Ok(false);
    }
    write_if_changed(path, existing, next, dry_run)
}

fn read_json_config(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn ensure_json_server(config: &mut Value, binary_path: &Path) -> Result<()> {
    if !config.is_object() {
        *config = json!({});
    }
    let object = config
        .as_object_mut()
        .ok_or_else(|| anyhow!("JSON config root is not an object"))?;
    let servers = object
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow!("mcpServers must be a JSON object"))?;
    servers.insert(
        "mempalace".to_string(),
        json!({
            "command": path_to_string(binary_path),
            "args": ["mcp"],
        }),
    );
    Ok(())
}

fn write_if_changed(path: &Path, existing: Value, next: Value, dry_run: bool) -> Result<bool> {
    if existing == next {
        return Ok(false);
    }
    if dry_run {
        return Ok(true);
    }
    backup_existing(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let text = serde_json::to_string_pretty(&next)?;
    fs::write(path, format!("{text}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

fn write_codex_client(path: &Path, binary_path: &Path, dry_run: bool) -> Result<bool> {
    let existing = read_toml_document(path)?;
    let mut next = existing.clone();
    ensure_codex_server(&mut next, binary_path);
    write_toml_if_changed(path, &existing, &next, dry_run)
}

fn remove_codex_client(path: &Path, dry_run: bool) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = read_toml_document(path)?;
    let mut next = existing.clone();
    let Some(servers) = next
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
    else {
        return Ok(false);
    };
    if servers.remove("mempalace").is_none() {
        return Ok(false);
    }
    write_toml_if_changed(path, &existing, &next, dry_run)
}

fn read_toml_document(path: &Path) -> Result<DocumentMut> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    text.parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn ensure_codex_server(document: &mut DocumentMut, binary_path: &Path) {
    if !document.contains_key("mcp_servers") || !document["mcp_servers"].is_table_like() {
        document["mcp_servers"] = Item::Table(Table::new());
    }

    let mut server = Table::new();
    server["command"] = value(path_to_string(binary_path));
    let mut args = Array::new();
    args.push("mcp");
    server["args"] = value(args);
    document["mcp_servers"]["mempalace"] = Item::Table(server);
}

fn write_toml_if_changed(
    path: &Path,
    existing: &DocumentMut,
    next: &DocumentMut,
    dry_run: bool,
) -> Result<bool> {
    if existing.to_string() == next.to_string() {
        return Ok(false);
    }
    if dry_run {
        return Ok(true);
    }
    backup_existing(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, next.to_string())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

fn install_rule(target: &RuleTarget, dry_run: bool) -> Result<bool> {
    let existing = read_text_file(&target.path)?;
    let next = match target.kind {
        RuleKind::Standalone => cursor_rule_text(),
        RuleKind::ManagedBlock => upsert_managed_rule(&existing)?,
    };
    write_text_if_changed(&target.path, &existing, &next, dry_run)
}

fn uninstall_rule(target: &RuleTarget, dry_run: bool) -> Result<bool> {
    if !target.path.exists() {
        return Ok(false);
    }
    let existing = read_text_file(&target.path)?;
    let next = match target.kind {
        RuleKind::Standalone => String::new(),
        RuleKind::ManagedBlock => remove_managed_rule(&existing)?,
    };
    if target.kind == RuleKind::Standalone {
        if dry_run {
            return Ok(true);
        }
        backup_existing(&target.path)?;
        fs::remove_file(&target.path)
            .with_context(|| format!("failed to remove {}", target.path.display()))?;
        return Ok(true);
    }
    write_text_if_changed(&target.path, &existing, &next, dry_run)
}

fn rule_installed(target: &RuleTarget) -> Result<bool> {
    if !target.path.exists() {
        return Ok(false);
    }
    let text = read_text_file(&target.path)?;
    Ok(match target.kind {
        RuleKind::Standalone => text == cursor_rule_text(),
        RuleKind::ManagedBlock => find_managed_block(&text)?.is_some(),
    })
}

fn read_text_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn write_text_if_changed(path: &Path, existing: &str, next: &str, dry_run: bool) -> Result<bool> {
    if existing == next {
        return Ok(false);
    }
    if dry_run {
        return Ok(true);
    }
    backup_existing(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("rule path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, next).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

fn cursor_rule_text() -> String {
    format!(
        "---\ndescription: Consult MemPalace memory before answering about remembered facts\nalwaysApply: true\n---\n\n# MemPalace Memory Protocol — MANDATORY\n\n{RULE_BODY}\n"
    )
}

fn managed_rule_block() -> String {
    format!("{RULE_BEGIN}\n# MemPalace Memory Protocol — MANDATORY\n\n{RULE_BODY}\n{RULE_END}")
}

fn upsert_managed_rule(existing: &str) -> Result<String> {
    let block = managed_rule_block();
    if let Some((start, end)) = find_managed_block(existing)? {
        let mut next = String::with_capacity(existing.len() + block.len());
        next.push_str(&existing[..start]);
        next.push_str(&block);
        next.push_str(&existing[end..]);
        return Ok(next);
    }

    if existing.is_empty() {
        return Ok(format!("{block}\n"));
    }

    let separator = if existing.ends_with("\n\n") {
        ""
    } else if existing.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };
    Ok(format!("{existing}{separator}{block}\n"))
}

fn remove_managed_rule(existing: &str) -> Result<String> {
    let Some((start, end)) = find_managed_block(existing)? else {
        return Ok(existing.to_string());
    };
    let mut next = String::with_capacity(existing.len());
    next.push_str(&existing[..start]);
    next.push_str(&existing[end..]);
    while next.contains("\n\n\n") {
        next = next.replace("\n\n\n", "\n\n");
    }
    Ok(next)
}

fn find_managed_block(text: &str) -> Result<Option<(usize, usize)>> {
    let Some(start) = text.find(RULE_BEGIN) else {
        return Ok(None);
    };
    let search_from = start + RULE_BEGIN.len();
    let end_relative = text[search_from..]
        .find(RULE_END)
        .ok_or_else(|| anyhow!("managed MemPalace rule block is missing end marker"))?;
    let end = search_from + end_relative + RULE_END.len();
    Ok(Some((start, end)))
}

fn backup_existing(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let backup = path.with_file_name(format!(
        "{}.bak",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("invalid config filename: {}", path.display()))?
    ));
    if !backup.exists() {
        fs::copy(path, &backup).with_context(|| {
            format!(
                "failed to back up {} to {}",
                path.display(),
                backup.display()
            )
        })?;
    }
    Ok(())
}

fn read_configured_command(client: Client, path: &Path) -> Result<Option<String>> {
    match client {
        Client::Cursor | Client::Claude | Client::ClaudeDesktop => {
            if !path.exists() {
                return Ok(None);
            }
            let config = read_json_config(path)?;
            Ok(config
                .get("mcpServers")
                .and_then(|servers| servers.get("mempalace"))
                .and_then(|server| server.get("command"))
                .and_then(Value::as_str)
                .map(String::from))
        }
        Client::Codex => {
            if !path.exists() {
                return Ok(None);
            }
            let config = read_toml_document(path)?;
            Ok(config
                .get("mcp_servers")
                .and_then(|servers| servers.get("mempalace"))
                .and_then(|server| server.get("command"))
                .and_then(Item::as_str)
                .map(String::from))
        }
        Client::All => Ok(None),
    }
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
