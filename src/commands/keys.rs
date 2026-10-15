use crate::cli::KeysCommand;
use crate::cms::{ApiKeyItem, Cms};
use crate::config::{update_env_file, Config};
use crate::error::AppError;
use crate::prompt;
use crate::timeutil::{parse_rfc3339, seoul_short};

/// 키 관리는 키로는 안 되고 ID/비밀번호 로그인이 필요하다. 설정에 있으면 쓰고, 없으면 터미널에서 묻는다
/// (비밀번호는 입력이 화면에 보이지 않고 어디에도 저장하지 않는다).
fn login(config: &Config) -> Result<Cms, AppError> {
    if let Some((id, password)) = config.cms_credentials() {
        return Cms::login_with(config.cms_api_base(), id, password);
    }
    if !prompt::is_interactive() {
        return Err(AppError::MissingConfig("TYANGE_CMS_ID / TYANGE_CMS_PASSWORD"));
    }
    eprintln!("키 관리는 ID/비밀번호 로그인이 필요합니다(저장하지 않습니다).");
    let id = prompt::read_line("CMS ID: ")?;
    let password = prompt::read_secret("비밀번호(입력은 보이지 않습니다): ")?;
    if id.is_empty() || password.is_empty() {
        return Err(AppError::Invalid("ID와 비밀번호를 입력해야 합니다".into()));
    }
    Cms::login_with(config.cms_api_base(), &id, &password)
}

fn when(iso: &str) -> String {
    parse_rfc3339(iso).map(seoul_short).unwrap_or_else(|| iso.to_string())
}

pub fn status_label(key: &ApiKeyItem) -> String {
    match &key.revoked_at {
        Some(at) => format!("폐기됨 ({})", when(at)),
        None => "사용 중".to_string(),
    }
}

pub fn key_line(key: &ApiKeyItem) -> String {
    format!(
        "{:<4} {:<6} {:<18} {:<11} {:<11} {:<20} {}",
        key.api_key_id,
        key.role,
        format!("{}…", key.key_prefix),
        when(&key.created_at),
        key.last_used_at.as_deref().map(when).unwrap_or_else(|| "안 씀".into()),
        status_label(key),
        key.name
    )
}

pub fn run(config: &Config, command: KeysCommand, json: bool) -> Result<(), AppError> {
    let cms = login(config)?;
    match command {
        KeysCommand::List => {
            let keys = cms.list_api_keys()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&keys)?);
            } else if keys.is_empty() {
                println!("API 키가 없습니다.");
            } else {
                println!("{:<4} {:<6} {:<18} {:<11} {:<11} {:<20} {}", "id", "권한", "키", "만든 때", "마지막 사용", "상태", "이름");
                for key in &keys {
                    println!("{}", key_line(key));
                }
            }
        }
        KeysCommand::Create { name, role, save } => {
            let created = cms.create_api_key(&name, &role)?;
            if save {
                update_env_file(&[("TYANGE_CMS_API_KEY", Some(&created.key))])?;
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&created)?);
            } else {
                println!("{}", created.key);
                eprintln!(
                    "\n위 키는 지금만 볼 수 있습니다. {}",
                    if save {
                        "이 기기의 설정에 저장했습니다."
                    } else {
                        "`tyange auth login`으로 저장하거나 안전한 곳에 보관하세요."
                    }
                );
            }
        }
        KeysCommand::Revoke { id } => {
            cms.revoke_api_key(id)?;
            if json {
                println!("{}", serde_json::json!({ "revoked": id }));
            } else {
                println!("키 {id}을(를) 폐기했습니다.");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(revoked: Option<&str>, last_used: Option<&str>) -> ApiKeyItem {
        ApiKeyItem {
            api_key_id: 4,
            name: "tyange CLI".into(),
            role: "admin".into(),
            key_prefix: "tyk_f77c700ccf7b".into(),
            created_at: "2026-10-15T05:21:04Z".into(),
            last_used_at: last_used.map(str::to_string),
            revoked_at: revoked.map(str::to_string),
        }
    }

    #[test]
    fn a_key_line_shows_seoul_times_and_never_more_than_the_prefix() {
        let line = key_line(&item(None, Some("2026-10-15T05:21:29Z")));
        assert!(line.contains("10/15 14:21"), "{line}");
        assert!(line.contains("tyk_f77c700ccf7b…"), "{line}");
        assert!(line.contains("사용 중") && line.ends_with("tyange CLI"), "{line}");
    }

    #[test]
    fn unused_and_revoked_keys_say_so() {
        assert!(key_line(&item(None, None)).contains("안 씀"));
        let revoked = item(Some("2026-10-16T00:00:00Z"), None);
        assert_eq!(status_label(&revoked), "폐기됨 (10/16 09:00)");
    }
}
