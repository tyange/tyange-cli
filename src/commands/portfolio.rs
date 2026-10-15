use std::path::Path;

use serde_json::Value;

use crate::cli::{PortfolioCommand, SectionCommand};
use crate::cms::Cms;
use crate::error::AppError;

/// JSON 파일을 읽는다. 포트폴리오 문서와 섹션은 모두 객체여야 한다.
pub fn read_json_object(path: &Path) -> Result<Value, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Io(format!("{}: {e}", path.display())))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        AppError::Invalid(format!("{}이(가) 올바른 JSON이 아닙니다: {e}", path.display()))
    })?;
    if !value.is_object() {
        return Err(AppError::Invalid(format!(
            "{}의 최상위 값은 JSON 객체({{ … }})여야 합니다",
            path.display()
        )));
    }
    Ok(value)
}

fn print_pretty(value: &Value) -> Result<(), AppError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// 응답에서 `content`만 꺼낸다(고쳐서 `put`/`set`에 그대로 넣을 수 있게).
fn content_of(document: &Value) -> Result<&Value, AppError> {
    document
        .get("content")
        .ok_or_else(|| AppError::Invalid("응답에 content가 없습니다".into()))
}

pub fn run(cms: &Cms, command: PortfolioCommand) -> Result<(), AppError> {
    match command {
        PortfolioCommand::Get => {
            let document = cms.portfolio()?;
            print_pretty(content_of(&document)?)
        }
        PortfolioCommand::Put { file } => {
            let content = read_json_object(&file)?;
            cms.put_portfolio(&content)?;
            println!("포트폴리오를 저장했습니다.");
            Ok(())
        }
        PortfolioCommand::Delete { yes } => {
            if !yes {
                return Err(AppError::Invalid(
                    "포트폴리오 전체를 지웁니다. 되돌릴 수 없으니 정말이면 --yes를 붙이세요(먼저 `portfolio get > backup.json`으로 백업하세요)".into(),
                ));
            }
            cms.delete_portfolio()?;
            println!("포트폴리오를 지웠습니다.");
            Ok(())
        }
        PortfolioCommand::Section { command } => match command {
            SectionCommand::Get { key } => {
                let section = cms.section(&key)?;
                if let Some(updated) = section.get("updated_at").and_then(Value::as_str) {
                    eprintln!("{key} (마지막 저장 {updated})");
                }
                print_pretty(content_of(&section)?)
            }
            SectionCommand::Set { key, file } => {
                crate::cms::validate_section_key(&key)?;
                let content = read_json_object(&file)?;
                let saved = cms.put_section(&key, &content)?;
                let when = saved
                    .get("updated_at")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                println!("{key} 섹션을 저장했습니다 {when}");
                Ok(())
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, text: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("tyange-cli-test-{}-{name}", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path
    }

    #[test]
    fn a_json_object_file_is_read() {
        let path = write("ok.json", r#"{"slug":"dev","version":1}"#);
        let value = read_json_object(&path).unwrap();
        assert_eq!(value["slug"], "dev");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_or_non_object_json_is_rejected_with_the_path_in_the_message() {
        let bad = write("bad.json", "{ not json");
        let array = write("array.json", "[1,2]");
        for path in [&bad, &array] {
            let error = read_json_object(path).unwrap_err().to_string();
            assert!(error.contains(&path.display().to_string()), "{error}");
            std::fs::remove_file(path).unwrap();
        }
        assert!(read_json_object(Path::new("/no/such/file.json")).is_err());
    }

    #[test]
    fn content_is_what_gets_printed_so_get_and_put_round_trip() {
        let document = serde_json::json!({"portfolio_id": 1, "content": {"slug": "dev"}, "updated_at": "x"});
        assert_eq!(content_of(&document).unwrap(), &serde_json::json!({"slug": "dev"}));
        assert!(content_of(&serde_json::json!({})).is_err());
    }
}
