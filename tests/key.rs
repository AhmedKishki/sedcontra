use std::path::{Path, PathBuf};

use cross_examine::config::ProviderConfig;
use cross_examine::secret::{load_key, store_key};

const SECRET: &str = "sk-test-0123456789abcdef";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cross-examine-key-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("keys").join("deepseek")
}

fn cfg(file: &Path) -> ProviderConfig {
    ProviderConfig { base_url: "http://127.0.0.1:1".into(), api_key_env: None, api_key_file: Some(file.to_path_buf()) }
}

#[test]
fn stored_key_is_owner_only_and_loads() {
    let path = scratch("store");
    store_key(&path, SECRET).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    assert_eq!(load_key("deepseek", &cfg(&path)).unwrap().expose(), SECRET);
}

#[test]
fn key_never_prints_and_is_redacted_from_error_text() {
    let path = scratch("redact");
    store_key(&path, SECRET).unwrap();
    let key = load_key("deepseek", &cfg(&path)).unwrap();
    assert!(!format!("{key:?} {key}").contains(SECRET));
    let echoed = format!(r#"{{"error": "invalid key {SECRET}", "header": "Bearer {SECRET}"}}"#);
    let redacted = key.redact(&echoed);
    assert!(!redacted.contains(SECRET), "{redacted}");
    assert!(redacted.contains("[redacted]"));
}

#[cfg(unix)]
#[test]
fn key_file_readable_by_others_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let path = scratch("loose");
    store_key(&path, SECRET).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    let err = load_key("deepseek", &cfg(&path)).unwrap_err().to_string();
    assert!(err.contains("chmod 600"), "{err}");
    assert!(!err.contains(SECRET));
}

#[test]
fn missing_key_points_to_key_set() {
    let path = scratch("missing");
    let err = load_key("deepseek", &cfg(&path)).unwrap_err().to_string();
    assert!(err.contains("cross-examine key set deepseek"), "{err}");
}
