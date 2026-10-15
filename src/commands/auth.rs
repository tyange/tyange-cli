use crate::cli::AuthCommand;
use crate::cms::{key_prefix, Cms, API_KEY_PREFIX};
use crate::config::{update_env_file, Config};
use crate::error::AppError;

fn read_key() -> Result<String, AppError> {
    crate::prompt::read_secret("API 키(입력은 보이지 않습니다): ")
}

/// 이 키로 서버가 받아 주는지 가볍게 확인한다(관리자 전용 상태 조회).
fn verify(cms: &Cms) -> Result<(), AppError> {
    cms.craft_status().map(|_| ())
}

fn describe_failure(error: &AppError) -> String {
    match error {
        AppError::Api { status: 401, .. } => "서버가 이 키를 받아 주지 않습니다(없거나 폐기된 키)".into(),
        AppError::Api { status: 403, .. } => {
            "키는 유효하지만 admin 권한이 아닙니다(CMS에서 admin 키를 만드세요)".into()
        }
        other => other.to_string(),
    }
}

pub fn run(command: AuthCommand, json: bool, config: &Config) -> Result<(), AppError> {
    match command {
        AuthCommand::Login { keep_password } => {
            let key = read_key()?;
            if !key.starts_with(API_KEY_PREFIX) {
                return Err(AppError::Invalid(format!(
                    "API 키는 `{API_KEY_PREFIX}`로 시작합니다. CMS의 API 키 화면에서 만든 값을 넣어 주세요"
                )));
            }

            // 저장하기 전에 서버가 받아 주는지 확인한다. 안 되는 키로 기존 설정을 덮어쓰지 않는다.
            let cms = Cms::with_key(config.cms_api_base(), &key)?;
            verify(&cms).map_err(|e| AppError::Invalid(describe_failure(&e)))?;

            let mut updates: Vec<(&str, Option<&str>)> = vec![("TYANGE_CMS_API_KEY", Some(&key))];
            if !keep_password {
                updates.push(("TYANGE_CMS_ID", None));
                updates.push(("TYANGE_CMS_PASSWORD", None));
            }
            let path = update_env_file(&updates)?;

            if json {
                println!(
                    "{}",
                    serde_json::json!({ "saved": true, "key": key_prefix(&key), "file": path })
                );
            } else {
                println!("저장했습니다: {} ({})", key_prefix(&key), path.display());
                if !keep_password {
                    println!("저장돼 있던 ID/비밀번호는 지웠습니다(--keep-password로 남길 수 있습니다).");
                }
            }
        }
        AuthCommand::Status => {
            let (method, detail, cms) = match config.cms_api_key() {
                Some(key) => ("api-key", Some(key_prefix(key)), Cms::with_key(config.cms_api_base(), key)),
                None if config.require("TYANGE_CMS_PASSWORD").is_ok() => {
                    ("password", None, Cms::login(config))
                }
                None => {
                    if json {
                        println!("{}", serde_json::json!({ "method": null, "ok": false }));
                    } else {
                        println!("로그인 설정이 없습니다. `tyange auth login`으로 API 키를 저장하세요.");
                    }
                    return Ok(());
                }
            };

            let result = cms.and_then(|cms| verify(&cms));
            let ok = result.is_ok();
            let problem = result.as_ref().err().map(describe_failure);

            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "method": method,
                        "key": detail,
                        "server": config.cms_api_base(),
                        "ok": ok,
                        "problem": problem,
                    })
                );
            } else {
                let label = match (method, &detail) {
                    ("api-key", Some(prefix)) => format!("API 키 ({prefix})"),
                    _ => "ID/비밀번호 로그인".to_string(),
                };
                println!("방식:   {label}");
                println!("서버:   {}", config.cms_api_base());
                match &problem {
                    None => println!("확인:   서버가 받아 줍니다"),
                    Some(problem) => println!("확인:   실패 — {problem}"),
                }
            }
            if !ok {
                return Err(AppError::Invalid("로그인 확인에 실패했습니다".into()));
            }
        }
        AuthCommand::Logout => {
            let path = update_env_file(&[("TYANGE_CMS_API_KEY", None)])?;
            if json {
                println!("{}", serde_json::json!({ "removed": true }));
            } else {
                println!("저장된 API 키를 지웠습니다 ({}).", path.display());
                println!("키 자체는 서버에 남아 있습니다. 폐기하려면 CMS의 API 키 화면에서 하세요.");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_are_explained_in_terms_the_user_can_act_on() {
        let unauthorized = AppError::Api { status: 401, message: "x".into() };
        let forbidden = AppError::Api { status: 403, message: "x".into() };
        assert!(describe_failure(&unauthorized).contains("폐기"));
        assert!(describe_failure(&forbidden).contains("admin"));
        assert!(describe_failure(&AppError::Network("timeout".into())).contains("timeout"));
    }
}
