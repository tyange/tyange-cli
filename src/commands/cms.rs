use std::io::Read;

use serde_json::Value;

use crate::cli::CmsCommand;
use crate::cms::{parse_tag, today_kst, Cms, Post, PostBody, PostItem, Tag};
use crate::config::Config;
use crate::error::AppError;

fn clip(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// 목록에서 글을 알아보게 하는 한 줄. 제목이 없으면(lovelog 짧은 글) 본문 앞부분을 쓴다.
pub fn label(item: &PostItem) -> String {
    if !item.title.trim().is_empty() {
        return clip(item.title.trim(), 50);
    }
    let excerpt = item.excerpt.as_deref().unwrap_or("").trim();
    if excerpt.starts_with("![") && excerpt.ends_with(')') && !excerpt.contains('\n') {
        return "(이미지)".to_string();
    }
    let one_line = excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        "(제목 없음)".to_string()
    } else {
        clip(&one_line, 50)
    }
}

pub fn post_line(item: &PostItem) -> String {
    let tags = item
        .tags
        .iter()
        .map(|t| t.tag.as_str())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}  {:<9}  {}  [{}]  {}",
        item.post_id.chars().take(8).collect::<String>(),
        item.status,
        item.published_at,
        tags,
        label(item)
    )
}

fn read_content(file: &str) -> Result<String, AppError> {
    if file == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        return Ok(text);
    }
    Ok(std::fs::read_to_string(file)?)
}

fn parse_tags(inputs: &[String]) -> Result<Vec<Tag>, AppError> {
    inputs.iter().map(|input| parse_tag(input)).collect()
}

fn print_json(value: &impl serde::Serialize) -> Result<(), AppError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_post(post: &Post) {
    let tags = post
        .tags
        .iter()
        .map(|t| format!("{}/{}", t.category, t.tag))
        .collect::<Vec<_>>()
        .join(", ");
    println!("id:        {}", post.post_id);
    println!("status:    {}", post.status);
    println!("published: {}", post.published_at);
    println!("created:   {}", post.created_at);
    println!("title:     {}", post.title);
    println!("tags:      {tags}");
    println!("\n{}", post.content);
}

fn names(report: &Value, key: &str) -> Vec<String> {
    report
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let title = item.get("title").and_then(Value::as_str).unwrap_or("");
                    let id = item.get("post_id").and_then(Value::as_str).unwrap_or("");
                    format!("{} ({})", if title.is_empty() { "(제목 없음)" } else { title }, id)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn print_sync_report(report: &Value) {
    for (key, label) in [("created", "새 글"), ("updated", "갱신"), ("drafted", "draft로 전환")] {
        for name in names(report, key) {
            println!("{label}: {name}");
        }
    }
    let unchanged = report.get("unchanged").and_then(Value::as_u64).unwrap_or(0);
    println!("바뀐 게 없는 문서: {unchanged}개");
    for key in ["warnings", "errors"] {
        for item in report.get(key).and_then(Value::as_array).into_iter().flatten() {
            println!("{}: {}", if key == "errors" { "오류" } else { "경고" }, item.as_str().unwrap_or(""));
        }
    }
}

pub fn run(command: CmsCommand, json: bool, config: &Config) -> Result<(), AppError> {
    let cms = Cms::connect(config)?;
    match command {
        CmsCommand::Posts { status, tag, limit } => {
            let posts: Vec<PostItem> = cms
                .list_posts()?
                .into_iter()
                .filter(|p| status.as_deref().is_none_or(|s| p.status == s))
                .filter(|p| tag.as_deref().is_none_or(|t| p.tags.iter().any(|x| x.tag == t)))
                .take(limit)
                .collect();
            if json {
                return print_json(&posts);
            }
            if posts.is_empty() {
                println!("조건에 맞는 글이 없습니다.");
            }
            for post in &posts {
                println!("{}", post_line(post));
            }
        }
        CmsCommand::Get { id } => {
            let post = cms.get_post(&id)?;
            if json {
                return print_json(&post);
            }
            print_post(&post);
        }
        CmsCommand::New { file, title, description, tags, status, date } => {
            let content = read_content(&file)?;
            if content.trim().is_empty() {
                return Err(AppError::Invalid("본문이 비어 있습니다".into()));
            }
            let body = PostBody {
                title,
                description,
                published_at: date.unwrap_or_else(today_kst),
                tags: parse_tags(&tags)?,
                content,
                status,
            };
            let id = cms.create_post(&body)?;
            if json {
                return print_json(&serde_json::json!({ "post_id": id, "status": body.status }));
            }
            println!("{id} ({})", body.status);
        }
        CmsCommand::Update { id, file, title, description, tags, status, date } => {
            if file.is_none()
                && title.is_none()
                && description.is_none()
                && tags.is_empty()
                && status.is_none()
                && date.is_none()
            {
                return Err(AppError::Invalid("바꿀 항목이 없습니다(--file, --title, --status 등)".into()));
            }
            let mut body = PostBody::from(cms.get_post(&id)?);
            if let Some(file) = file {
                body.content = std::fs::read_to_string(file)?;
            }
            if let Some(title) = title {
                body.title = title;
            }
            if let Some(description) = description {
                body.description = description;
            }
            if !tags.is_empty() {
                body.tags = parse_tags(&tags)?;
            }
            if let Some(status) = status {
                body.status = status;
            }
            if let Some(date) = date {
                body.published_at = date;
            }
            let post = cms.update_post(&id, &body)?;
            if json {
                return print_json(&post);
            }
            println!("{} ({})", post.post_id, post.status);
        }
        CmsCommand::Publish { id } => set_status(&cms, &id, "published", json)?,
        CmsCommand::Draft { id } => set_status(&cms, &id, "draft", json)?,
        CmsCommand::Delete { id, yes } => {
            if !yes {
                return Err(AppError::Invalid(
                    "삭제는 되돌릴 수 없습니다. 정말 지우려면 --yes를 붙이세요(draft로 돌리려면 `cms draft`)".into(),
                ));
            }
            cms.delete_post(&id)?;
            if json {
                return print_json(&serde_json::json!({ "deleted": id }));
            }
            println!("삭제했습니다: {id}");
        }
        CmsCommand::Sync => {
            let report = cms.craft_sync()?;
            if json {
                return print_json(&report);
            }
            print_sync_report(&report);
        }
        CmsCommand::SyncStatus => {
            let status = cms.craft_status()?;
            if json {
                return print_json(&status);
            }
            let enabled = status.get("enabled").and_then(Value::as_bool).unwrap_or(false);
            println!("Craft 동기화: {}", if enabled { "켜짐" } else { "꺼짐" });
            match status.get("last_report").filter(|r| !r.is_null()) {
                Some(report) => {
                    println!(
                        "마지막 실행: {}",
                        report.get("finished_at").and_then(Value::as_str).unwrap_or("?")
                    );
                    print_sync_report(report);
                }
                None => println!("아직 실행 기록이 없습니다."),
            }
        }
    }
    Ok(())
}

fn set_status(cms: &Cms, id: &str, status: &str, json: bool) -> Result<(), AppError> {
    let mut body = PostBody::from(cms.get_post(id)?);
    body.status = status.to_string();
    let post = cms.update_post(id, &body)?;
    if json {
        return print_json(&post);
    }
    println!("{} ({})", post.post_id, post.status);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(title: &str, excerpt: Option<&str>) -> PostItem {
        PostItem {
            post_id: "38a4761f-c294-4f71-be59-65211ef554e8".into(),
            title: title.into(),
            published_at: "2026-10-15".into(),
            created_at: String::new(),
            tags: vec![Tag { tag: "lovelog".into(), category: "general".into() }],
            status: "published".into(),
            excerpt: excerpt.map(str::to_string),
        }
    }

    #[test]
    fn untitled_posts_are_recognised_by_their_excerpt() {
        assert_eq!(label(&item("", Some("![](https://x/api/cms/images/a.png)"))), "(이미지)");
        assert_eq!(label(&item("", Some("첫 줄\n\n둘째   줄"))), "첫 줄 둘째 줄");
        assert_eq!(label(&item("", None)), "(제목 없음)");
        assert_eq!(label(&item("제목", Some("본문"))), "제목");
    }

    #[test]
    fn long_labels_are_clipped() {
        let long = "가".repeat(80);
        assert_eq!(label(&item("", Some(&long))).chars().count(), 51);
    }

    #[test]
    fn a_post_line_has_the_short_id_status_date_and_tags() {
        let line = post_line(&item("", Some("![](x)")));
        assert!(line.starts_with("38a4761f  published  2026-10-15  [lovelog]  (이미지)"), "{line}");
    }

    #[test]
    fn sync_report_names_use_title_and_id() {
        let report = serde_json::json!({
            "created": [{"title": "연보", "post_id": "p1", "craft_doc_id": "d"}],
            "drafted": [{"title": "", "post_id": "p2", "craft_doc_id": "e"}],
        });
        assert_eq!(names(&report, "created"), vec!["연보 (p1)".to_string()]);
        assert_eq!(names(&report, "drafted"), vec!["(제목 없음) (p2)".to_string()]);
        assert!(names(&report, "updated").is_empty());
    }
}
