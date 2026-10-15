use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Config;
use crate::error::AppError;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Tag {
    pub tag: String,
    pub category: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Post {
    pub post_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub content: String,
    pub status: String,
}

/// 목록용. 본문 대신 앞 300자(`excerpt`)만 온다.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostItem {
    pub post_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
    pub status: String,
    #[serde(default)]
    pub excerpt: Option<String>,
}

/// 새 글·수정에 보내는 본문. 서버는 항상 전체 필드를 요구한다(PUT).
#[derive(Debug, Clone, Serialize)]
pub struct PostBody {
    pub title: String,
    pub description: String,
    pub published_at: String,
    pub tags: Vec<Tag>,
    pub content: String,
    pub status: String,
}

impl From<Post> for PostBody {
    fn from(post: Post) -> Self {
        PostBody {
            title: post.title,
            description: post.description,
            published_at: post.published_at,
            tags: post.tags,
            content: post.content,
            status: post.status,
        }
    }
}

/// 서버는 `{status, data, message}`로 감싸 돌려준다. 단, 공개 `GET /post/:id`만 봉투 없이 글 그대로다.
pub fn unwrap_data(body: Value) -> Value {
    match body {
        Value::Object(mut map) if map.contains_key("data") && map.contains_key("status") => {
            map.remove("data").unwrap_or(Value::Null)
        }
        other => other,
    }
}

/// `category/name`이나 `name`(카테고리는 `general`)을 태그로 읽는다.
pub fn parse_tag(input: &str) -> Result<Tag, AppError> {
    let (category, tag) = match input.split_once('/') {
        Some((category, tag)) => (category.trim(), tag.trim()),
        None => ("general", input.trim()),
    };
    if tag.is_empty() || category.is_empty() {
        return Err(AppError::Invalid(format!("태그를 읽지 못했습니다: '{input}'")));
    }
    Ok(Tag {
        tag: tag.to_string(),
        category: category.to_string(),
    })
}

pub struct Cms {
    http: Client,
    base: String,
    /// `Authorization` 헤더에 그대로 넣는 값. 키는 `Bearer <키>`, 로그인 JWT는 토큰만(서버가 옛 방식으로 읽는다).
    authorization: String,
}

pub const API_KEY_PREFIX: &str = "tyk_";

/// 키를 목록·상태에 보여 줄 때 쓰는 앞부분(`tyk_` + 12자). 이것만으론 인증되지 않는다.
pub fn key_prefix(key: &str) -> String {
    format!("{}…", key.chars().take(16).collect::<String>())
}

impl Cms {
    /// API 키가 설정돼 있으면 키로(로그인 요청 없음), 아니면 ID/비밀번호로 로그인한다.
    pub fn connect(config: &Config) -> Result<Self, AppError> {
        match config.cms_api_key() {
            Some(key) => Self::with_key(config.cms_api_base(), key),
            None => Self::login(config),
        }
    }

    pub fn with_key(base: &str, key: &str) -> Result<Self, AppError> {
        if !key.starts_with(API_KEY_PREFIX) {
            return Err(AppError::Invalid(format!(
                "API 키는 `{API_KEY_PREFIX}`로 시작해야 합니다"
            )));
        }
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(30)).build()?,
            base: base.trim_end_matches('/').to_string(),
            authorization: format!("Bearer {key}"),
        })
    }

    /// 로그인해서 토큰을 받는다. 비밀번호는 이 호출에만 쓰이고 어디에도 남기지 않는다.
    pub fn login(config: &Config) -> Result<Self, AppError> {
        let id = config.require("TYANGE_CMS_ID")?;
        let password = config.require("TYANGE_CMS_PASSWORD")?;
        let base = config.cms_api_base().to_string();
        let http = Client::builder().timeout(Duration::from_secs(30)).build()?;

        let response = http
            .post(format!("{base}/login"))
            .json(&serde_json::json!({ "user_id": id, "password": password }))
            .send()?;
        let body: Value = Self::read(response)?;
        let token = body
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Invalid("로그인 응답에 access_token이 없습니다".into()))?
            .to_string();
        Ok(Self {
            http,
            base,
            authorization: token,
        })
    }

    fn read<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, AppError> {
        let status = response.status();
        let text = response.text()?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                .unwrap_or_else(|| text.chars().take(200).collect());
            return Err(AppError::Api {
                status: status.as_u16(),
                message,
            });
        }
        Ok(serde_json::from_str(&text)?)
    }

    /// 인증 헤더는 `Bearer` 없이 토큰만 넣는다(서버 `AuthGuard`가 그렇게 읽는다).
    fn send<T: for<'de> Deserialize<'de>>(&self, request: RequestBuilder) -> Result<T, AppError> {
        Self::read(request.header("Authorization", &self.authorization).send()?)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// draft 포함 전체 목록(관리자). 최신순.
    pub fn list_posts(&self) -> Result<Vec<PostItem>, AppError> {
        let body: Value = self.send(self.http.get(self.url("/admin/posts")))?;
        let posts = unwrap_data(body)
            .get("posts")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        Ok(serde_json::from_value(posts)?)
    }

    pub fn get_post(&self, id: &str) -> Result<Post, AppError> {
        let body: Value = self.send(self.http.get(self.url(&format!("/post/{id}"))))?;
        Ok(serde_json::from_value(unwrap_data(body))?)
    }

    pub fn create_post(&self, post: &PostBody) -> Result<String, AppError> {
        let body: Value = self.send(self.http.post(self.url("/post/upload")).json(post))?;
        unwrap_data(body)
            .get("post_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| AppError::Invalid("응답에 post_id가 없습니다".into()))
    }

    pub fn update_post(&self, id: &str, post: &PostBody) -> Result<Post, AppError> {
        let body: Value = self.send(
            self.http
                .put(self.url(&format!("/post/update/{id}")))
                .json(post),
        )?;
        Ok(serde_json::from_value(unwrap_data(body))?)
    }

    pub fn delete_post(&self, id: &str) -> Result<(), AppError> {
        let _: Value = self.send(self.http.delete(self.url(&format!("/post/delete/{id}"))))?;
        Ok(())
    }

    /// 서버 주소(검증·상태 출력용).
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Craft 폴더를 지금 바로 한 번 동기화한다.
    pub fn craft_sync(&self) -> Result<Value, AppError> {
        let body: Value = self.send(self.http.post(self.url("/admin/craft/sync")))?;
        Ok(unwrap_data(body))
    }

    pub fn craft_status(&self) -> Result<Value, AppError> {
        let body: Value = self.send(self.http.get(self.url("/admin/craft/status")))?;
        Ok(unwrap_data(body))
    }
}

/// 오늘 날짜(한국 시각, `YYYY-MM-DD`). 서버의 `published_at`이 이 모양이다.
pub fn kst_date(unix_secs: i64) -> String {
    let days = (unix_secs + 9 * 3600).div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn today_kst() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    kst_date(now)
}

/// 1970-01-01로부터의 일수 → (년, 월, 일). Howard Hinnant의 civil_from_days.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kst_date_rolls_over_at_utc_15() {
        assert_eq!(kst_date(1_792_076_399), "2026-10-15");
        assert_eq!(kst_date(1_792_076_400), "2026-10-16");
        assert_eq!(kst_date(1_767_225_600), "2026-01-01");
        assert_eq!(kst_date(0), "1970-01-01");
    }

    #[test]
    fn unwrap_data_strips_the_envelope_only_when_there_is_one() {
        let wrapped = serde_json::json!({"status": true, "data": {"post_id": "a"}, "message": null});
        assert_eq!(unwrap_data(wrapped), serde_json::json!({"post_id": "a"}));
        let bare = serde_json::json!({"post_id": "a", "status": "published", "content": "x"});
        assert_eq!(unwrap_data(bare.clone()), bare, "글 자체의 status 필드를 봉투로 오해하면 안 된다");
    }

    #[test]
    fn tags_default_to_the_general_category() {
        assert_eq!(
            parse_tag("lovelog").unwrap(),
            Tag { tag: "lovelog".into(), category: "general".into() }
        );
        assert_eq!(
            parse_tag("dev/rust").unwrap(),
            Tag { tag: "rust".into(), category: "dev".into() }
        );
        assert!(parse_tag("").is_err());
        assert!(parse_tag("dev/").is_err());
    }

    #[test]
    fn api_keys_are_sent_as_bearer_and_must_have_the_prefix() {
        let cms = Cms::with_key("http://x/", "tyk_0123456789ab_secret").unwrap();
        assert_eq!(cms.authorization, "Bearer tyk_0123456789ab_secret");
        assert_eq!(cms.base(), "http://x");
        assert!(Cms::with_key("http://x", "not-a-key").is_err());
    }

    #[test]
    fn the_key_prefix_never_includes_the_secret() {
        let shown = key_prefix("tyk_0123456789ab_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(shown, "tyk_0123456789ab…");
        assert!(!shown.contains("aaaa"));
    }

    #[test]
    fn a_post_round_trips_into_an_update_body() {
        let post: Post = serde_json::from_value(serde_json::json!({
            "post_id": "p1", "title": "", "description": "", "published_at": "2026-10-15",
            "created_at": "2026-10-15T03:34:32Z", "content": "본문", "status": "published",
            "tags": [{"tag": "lovelog", "category": "general"}]
        }))
        .unwrap();
        let body = serde_json::to_value(PostBody::from(post)).unwrap();
        assert_eq!(body["content"], "본문");
        assert_eq!(body["tags"][0]["tag"], "lovelog");
        assert!(body.get("post_id").is_none() && body.get("created_at").is_none());
    }
}
