use std::path::Path;

use crate::cms::Cms;
use crate::error::AppError;

/// `tyange cms tags` — 카테고리별 태그 목록.
pub fn tags(cms: &Cms, json: bool) -> Result<(), AppError> {
    let groups = cms.tags()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&groups)?);
        return Ok(());
    }
    if groups.is_empty() {
        println!("태그가 없습니다.");
    }
    for group in &groups {
        println!("{} ({}개)", group.category, group.tags.len());
        println!("  {}", group.tags.join(", "));
    }
    Ok(())
}

/// `tyange cms image upload` — 이미지를 올리고 본문에 붙일 주소를 알려 준다.
pub fn upload(
    cms: &Cms,
    file: &Path,
    post_id: Option<&str>,
    kind: &str,
    json: bool,
) -> Result<(), AppError> {
    let web_path = cms.upload_image(file, post_id, kind)?;
    let url = cms.image_url(&web_path);
    if json {
        println!(
            "{}",
            serde_json::json!({ "image_path": web_path, "url": url, "markdown": format!("![]({url})") })
        );
    } else {
        println!("{url}");
        if kind == "in_post" {
            println!("![]({url})");
        }
    }
    Ok(())
}
