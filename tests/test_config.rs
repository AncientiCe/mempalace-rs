use mempalace::config::MempalaceConfig;
use std::sync::Mutex;
use tempfile::TempDir;

// Serialise tests that touch env vars (env is process-global)
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn default_config_paths_are_stable() {
    let _guard = ENV_LOCK.lock().unwrap();
    // Ensure our special env var isn't set from a prior test
    std::env::remove_var("MEMPALACE_PALACE_PATH");
    let tmp = TempDir::new().unwrap();
    let config = MempalaceConfig::with_config_dir(Some(tmp.path()));
    assert_eq!(config.palace_path(), tmp.path().join("palace"));
    assert_eq!(config.palace_db_path(), tmp.path().join("palace/palace.db"));
    assert_eq!(config.collection_name(), "mempalace_drawers");
}

#[test]
fn init_creates_config_file() {
    let tmp = TempDir::new().unwrap();
    let config = MempalaceConfig::with_config_dir(Some(tmp.path()));
    let config_file = config.init().unwrap();
    assert!(config_file.exists());
    let content = std::fs::read_to_string(config_file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("palace_path").is_some());
}

#[test]
fn topic_wings_returns_defaults() {
    let tmp = TempDir::new().unwrap();
    let config = MempalaceConfig::with_config_dir(Some(tmp.path()));
    let wings = config.topic_wings();
    assert!(wings.contains(&"technical".to_string()));
    assert!(wings.contains(&"emotions".to_string()));
}

#[test]
fn env_var_overrides_palace_path() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    let custom_path = tmp.path().join("custom_palace");
    std::env::set_var("MEMPALACE_PALACE_PATH", custom_path.to_string_lossy().as_ref());
    let config = MempalaceConfig::with_config_dir(Some(tmp.path()));
    let result = config.palace_path();
    std::env::remove_var("MEMPALACE_PALACE_PATH");
    assert_eq!(result, custom_path);
}
