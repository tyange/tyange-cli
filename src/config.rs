use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::AppError;

pub const DEFAULT_CMS_API_BASE: &str = "https://tyange.com/api/cms";
pub const DEFAULT_FORGEJO_HOST: &str = "git.tyange.com";

/// 설정은 환경변수가 우선이고, 없으면 `~/.config/tyange-cli/env`(`KEY=VALUE` 줄)에서 읽는다.
/// 토큰·비밀번호는 어디에도 출력하지 않는다(`Debug`도 값을 가린다).
#[derive(Default)]
pub struct Config {
    values: HashMap<String, String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut keys: Vec<_> = self.values.keys().collect();
        keys.sort();
        write!(f, "Config {{ keys: {keys:?} }}")
    }
}

/// env 파일 텍스트에 키를 넣거나(`Some`) 뺀다(`None`). 나머지 줄·주석·순서는 그대로 둔다.
pub fn apply_env_updates(text: &str, updates: &[(&str, Option<&str>)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (key, value) in updates {
        let is_key = |line: &String| {
            line.trim()
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == *key)
        };
        match value {
            Some(value) => {
                let new_line = format!("{key}={value}");
                match lines.iter().position(is_key) {
                    Some(i) => lines[i] = new_line,
                    None => lines.push(new_line),
                }
                // 같은 키가 여러 줄이면 첫 줄만 남긴다.
                let mut seen = false;
                lines.retain(|line| !is_key(line) || !std::mem::replace(&mut seen, true));
            }
            None => lines.retain(|line| !is_key(line)),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// env 파일을 갱신한다. 없으면 만들고, 항상 소유자만 읽고 쓰게(0600) 둔다.
pub fn update_env_file(updates: &[(&str, Option<&str>)]) -> Result<PathBuf, AppError> {
    use std::os::unix::fs::PermissionsExt;

    let path = config_file_path().ok_or(AppError::MissingConfig("HOME"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    std::fs::write(&path, apply_env_updates(&existing, updates))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(path)
}

pub fn config_file_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/tyange-cli/env"))
}

/// `KEY=VALUE` 줄만 읽는다. 빈 줄·`#` 주석은 건너뛰고, 값 양끝의 따옴표는 벗긴다.
pub fn parse_env_file(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            Some((key.trim().to_string(), value.to_string()))
        })
        .collect()
}

impl Config {
    pub fn load() -> Config {
        let mut values = config_file_path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| parse_env_file(&text))
            .unwrap_or_default();
        for key in [
            "TYANGE_FORGEJO_TOKEN",
            "TYANGE_FORGEJO_HOST",
            "TYANGE_CMS_API_BASE",
            "TYANGE_CMS_API_KEY",
            "TYANGE_CMS_ID",
            "TYANGE_CMS_PASSWORD",
        ] {
            if let Ok(value) = std::env::var(key) {
                values.insert(key.to_string(), value);
            }
        }
        Config { values }
    }

    #[cfg(test)]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Config {
        Config {
            values: pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.trim().is_empty())
    }

    pub fn require(&self, key: &'static str) -> Result<&str, AppError> {
        self.get(key).ok_or(AppError::MissingConfig(key))
    }

    /// 있으면 비밀번호 로그인 대신 이 키를 쓴다.
    pub fn cms_api_key(&self) -> Option<&str> {
        self.get("TYANGE_CMS_API_KEY")
    }

    /// `ci --all`이 목록을 가져올 Forgejo 주소(호스트만). 저장소 폴더 안의 `ci`는 origin에서 알아낸다.
    pub fn forgejo_host(&self) -> &str {
        self.get("TYANGE_FORGEJO_HOST").unwrap_or(DEFAULT_FORGEJO_HOST)
    }

    pub fn forgejo_token(&self) -> Result<&str, AppError> {
        self.require("TYANGE_FORGEJO_TOKEN")
    }

    pub fn cms_api_base(&self) -> &str {
        self.get("TYANGE_CMS_API_BASE")
            .unwrap_or(DEFAULT_CMS_API_BASE)
            .trim_end_matches('/')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_key_value_lines() {
        let map = parse_env_file("# 주석\n\nA=1\nB = \"two\"\nC='3'\nbroken line\n");
        assert_eq!(map.get("A").map(String::as_str), Some("1"));
        assert_eq!(map.get("B").map(String::as_str), Some("two"));
        assert_eq!(map.get("C").map(String::as_str), Some("3"));
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn missing_or_blank_values_are_errors() {
        let config = Config::from_pairs(&[("TYANGE_CMS_ID", "  ")]);
        assert!(matches!(
            config.require("TYANGE_CMS_ID"),
            Err(AppError::MissingConfig("TYANGE_CMS_ID"))
        ));
        assert!(config.forgejo_token().is_err());
    }

    #[test]
    fn cms_base_defaults_and_trims_the_trailing_slash() {
        assert_eq!(Config::default().cms_api_base(), DEFAULT_CMS_API_BASE);
        let config = Config::from_pairs(&[("TYANGE_CMS_API_BASE", "http://localhost:8080/")]);
        assert_eq!(config.cms_api_base(), "http://localhost:8080");
    }

    #[test]
    fn env_updates_replace_append_and_remove_without_touching_other_lines() {
        let before = "# 내 설정\nTYANGE_CMS_ID=me\nTYANGE_CMS_PASSWORD=pw\nOTHER=1\n";
        let after = apply_env_updates(
            before,
            &[
                ("TYANGE_CMS_API_KEY", Some("tyk_new")),
                ("TYANGE_CMS_PASSWORD", None),
                ("TYANGE_CMS_ID", Some("you")),
            ],
        );
        assert_eq!(after, "# 내 설정\nTYANGE_CMS_ID=you\nOTHER=1\nTYANGE_CMS_API_KEY=tyk_new\n");
        let map = parse_env_file(&after);
        assert_eq!(map.get("TYANGE_CMS_API_KEY").map(String::as_str), Some("tyk_new"));
        assert!(!map.contains_key("TYANGE_CMS_PASSWORD"));
    }

    #[test]
    fn env_updates_dedupe_a_repeated_key_and_handle_an_empty_file() {
        assert_eq!(apply_env_updates("A=1\nA=2\n", &[("A", Some("3"))]), "A=3\n");
        assert_eq!(apply_env_updates("", &[("A", Some("1"))]), "A=1\n");
        assert_eq!(apply_env_updates("A=1\n", &[("B", None)]), "A=1\n");
    }

    #[test]
    fn debug_never_prints_values() {
        let config = Config::from_pairs(&[("TYANGE_CMS_PASSWORD", "hunter2")]);
        assert!(!format!("{config:?}").contains("hunter2"));
    }
}
