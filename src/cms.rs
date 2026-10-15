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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TagGroup {
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Guest {
    pub guest_id: i64,
    pub name: String,
    #[serde(default)]
    pub companions: i64,
    /// 방문 시간대의 시작 시각(24시간제). 시간대를 받기 전에 접수된 하객은 없다.
    #[serde(default)]
    pub visit_hour: Option<i64>,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuestList {
    #[serde(default)]
    pub guests: Vec<Guest>,
    #[serde(default)]
    pub total_guests: i64,
    #[serde(default)]
    pub total_attendees: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiKeyItem {
    pub api_key_id: i64,
    pub name: String,
    pub role: String,
    pub key_prefix: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreatedApiKey {
    pub api_key_id: i64,
    pub name: String,
    pub role: String,
    /// 키 원문. 만든 직후 응답에서만 볼 수 있다.
    pub key: String,
    #[serde(default)]
    pub created_at: String,
}

/// 포트폴리오 섹션 키. cms-api의 `validate_portfolio_section`과 같은 목록이다.
pub const SECTION_KEYS: [&str; 6] = [
    "meta",
    "identity",
    "intro",
    "featured_projects",
    "career",
    "portfolio_pdf",
];

pub fn validate_section_key(key: &str) -> Result<&str, AppError> {
    if SECTION_KEYS.contains(&key) {
        Ok(key)
    } else {
        Err(AppError::Invalid(format!(
            "알 수 없는 섹션입니다: '{key}' (가능: {})",
            SECTION_KEYS.join(", ")
        )))
    }
}

/// 서버의 이미지 업로드 상한(`AUTHENTICATED_UPLOAD_POLICY`)과 같은 값.
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

pub struct Cms {
    http: Client,
    base: String,
    /// `Authorization` 헤더에 그대로 넣는 값. 키는 `Bearer <키>`, 로그인 JWT는 토큰만(서버가 옛 방식으로 읽는다).
    authorization: String,
    /// 로그인 JWT로 연결했는가. 키 발급·폐기는 키로는 안 되고 로그인(JWT)이 필요하다.
    session: bool,
}

pub const API_KEY_PREFIX: &str = "tyk_";

/// 성공(2xx)이 아니면 서버가 준 메시지를 담은 에러. 본문이 JSON이 아니어도 앞 200자를 쓴다.
pub fn status_error(status: u16, body: &str) -> Option<AppError> {
    if (200..300).contains(&status) {
        return None;
    }
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    Some(AppError::Api { status, message })
}

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
            session: false,
        })
    }

    /// 설정의 ID/비밀번호로 로그인한다.
    pub fn login(config: &Config) -> Result<Self, AppError> {
        let id = config.require("TYANGE_CMS_ID")?;
        let password = config.require("TYANGE_CMS_PASSWORD")?;
        Self::login_with(config.cms_api_base(), id, password)
    }

    /// 로그인해서 토큰을 받는다. 비밀번호는 이 호출에만 쓰이고 어디에도 남기지 않는다.
    pub fn login_with(base: &str, id: &str, password: &str) -> Result<Self, AppError> {
        let base = base.trim_end_matches('/').to_string();
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
            session: true,
        })
    }

    fn read<T: for<'de> Deserialize<'de>>(response: Response) -> Result<T, AppError> {
        let status = response.status().as_u16();
        let text = response.text()?;
        if let Some(error) = status_error(status, &text) {
            return Err(error);
        }
        Ok(serde_json::from_str(&text)?)
    }

    fn send<T: for<'de> Deserialize<'de>>(&self, request: RequestBuilder) -> Result<T, AppError> {
        Self::read(request.header("Authorization", &self.authorization).send()?)
    }

    /// 본문을 읽지 않고 성공 여부만 본다(삭제처럼 응답 본문이 비어 있을 수 있는 동작).
    fn send_ok(&self, request: RequestBuilder) -> Result<(), AppError> {
        let response = request.header("Authorization", &self.authorization).send()?;
        let status = response.status().as_u16();
        let text = response.text()?;
        match status_error(status, &text) {
            Some(error) => Err(error),
            None => Ok(()),
        }
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
        self.send_ok(self.http.delete(self.url(&format!("/post/delete/{id}"))))
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

    /// 카테고리별 태그 목록.
    pub fn tags(&self) -> Result<Vec<TagGroup>, AppError> {
        let body: Value = self.send(self.http.get(self.url("/tags-with-category")))?;
        Ok(serde_json::from_value(unwrap_data(body))?)
    }

    /// 이미지를 올리고 서버 경로(`/images/<파일>`)를 돌려준다. `kind`는 `in_post`(기본)나 `portfolio_icon`.
    pub fn upload_image(
        &self,
        file: &std::path::Path,
        post_id: Option<&str>,
        kind: &str,
    ) -> Result<String, AppError> {
        let size = std::fs::metadata(file)?.len();
        if size == 0 {
            return Err(AppError::Invalid("비어 있는 파일입니다".into()));
        }
        if size > MAX_IMAGE_BYTES {
            return Err(AppError::Invalid(format!(
                "이미지가 {}MB를 넘습니다({size}바이트). 줄여서 다시 올려 주세요",
                MAX_IMAGE_BYTES / 1_048_576
            )));
        }
        let form = reqwest::blocking::multipart::Form::new().file("file", file)?;
        let mut request = self
            .http
            .post(self.url("/upload-image"))
            .query(&[("image_type", kind)]);
        if let Some(post_id) = post_id {
            request = request.query(&[("post_id", post_id)]);
        }
        let body: Value = self.send(request.multipart(form))?;
        unwrap_data(body)
            .get("image_path")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| AppError::Invalid("응답에 image_path가 없습니다".into()))
    }

    /// 서버가 돌려준 이미지 경로를 글 본문에 쓸 수 있는 전체 주소로.
    pub fn image_url(&self, web_path: &str) -> String {
        format!("{}{web_path}", self.base)
    }

    /// 공개 포트폴리오 문서(섹션을 조립한 것). `content`가 편집 대상이다.
    pub fn portfolio(&self) -> Result<Value, AppError> {
        let body: Value = self.send(self.http.get(self.url("/portfolio")))?;
        Ok(unwrap_data(body))
    }

    /// 포트폴리오 전체 문서를 업서트한다(최초 생성용. 이후는 섹션별 저장).
    pub fn put_portfolio(&self, content: &Value) -> Result<Value, AppError> {
        let body: Value = self.send(
            self.http
                .put(self.url("/portfolio/update"))
                .json(&serde_json::json!({ "content": content })),
        )?;
        Ok(unwrap_data(body))
    }

    pub fn delete_portfolio(&self) -> Result<(), AppError> {
        self.send_ok(self.http.delete(self.url("/portfolio")))
    }

    /// 섹션 하나(`{ section_key, updated_at, content }`). 저장된 적이 없으면 404.
    pub fn section(&self, key: &str) -> Result<Value, AppError> {
        let key = validate_section_key(key)?;
        let body: Value = self.send(self.http.get(self.url(&format!("/portfolio/sections/{key}"))))?;
        Ok(unwrap_data(body))
    }

    pub fn put_section(&self, key: &str, content: &Value) -> Result<Value, AppError> {
        let key = validate_section_key(key)?;
        let body: Value = self.send(
            self.http
                .put(self.url(&format!("/portfolio/sections/{key}")))
                .json(&serde_json::json!({ "content": content })),
        )?;
        Ok(unwrap_data(body))
    }

    pub fn guests(&self) -> Result<GuestList, AppError> {
        let body: Value = self.send(self.http.get(self.url("/admin/wedding/guests")))?;
        Ok(serde_json::from_value(unwrap_data(body))?)
    }

    fn require_session(&self) -> Result<(), AppError> {
        if self.session {
            Ok(())
        } else {
            Err(AppError::Invalid(
                "API 키는 키를 만들거나 폐기할 수 없습니다. ID/비밀번호로 로그인해야 합니다".into(),
            ))
        }
    }

    pub fn list_api_keys(&self) -> Result<Vec<ApiKeyItem>, AppError> {
        self.require_session()?;
        let body: Value = self.send(self.http.get(self.url("/admin/api-keys")))?;
        let keys = unwrap_data(body)
            .get("keys")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        Ok(serde_json::from_value(keys)?)
    }

    pub fn create_api_key(&self, name: &str, role: &str) -> Result<CreatedApiKey, AppError> {
        self.require_session()?;
        let body: Value = self.send(
            self.http
                .post(self.url("/admin/api-keys"))
                .json(&serde_json::json!({ "name": name, "role": role })),
        )?;
        Ok(serde_json::from_value(unwrap_data(body))?)
    }

    pub fn revoke_api_key(&self, id: i64) -> Result<(), AppError> {
        self.require_session()?;
        self.send_ok(self.http.delete(self.url(&format!("/admin/api-keys/{id}"))))
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
    fn success_is_any_2xx_even_with_an_empty_body_and_errors_carry_the_servers_message() {
        assert!(status_error(200, "").is_none(), "빈 본문 200(포트폴리오 삭제)도 성공이다");
        assert!(status_error(204, "").is_none());
        match status_error(400, r#"{"message":"이름이 비었습니다"}"#) {
            Some(AppError::Api { status: 400, message }) => assert_eq!(message, "이름이 비었습니다"),
            other => panic!("{other:?}"),
        }
        match status_error(502, "<html>bad gateway</html>") {
            Some(AppError::Api { status: 502, message }) => assert!(message.starts_with("<html>")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn section_keys_are_validated_against_the_servers_list() {
        for key in SECTION_KEYS {
            assert_eq!(validate_section_key(key).unwrap(), key);
        }
        let error = validate_section_key("nope").unwrap_err().to_string();
        assert!(error.contains("nope") && error.contains("portfolio_pdf"), "{error}");
    }

    #[test]
    fn an_api_key_connection_cannot_manage_keys() {
        let cms = Cms::with_key("http://127.0.0.1:1", "tyk_0123456789ab_secret").unwrap();
        for result in [
            cms.list_api_keys().map(|_| ()),
            cms.create_api_key("x", "admin").map(|_| ()),
            cms.revoke_api_key(1),
        ] {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("비밀번호로 로그인"), "네트워크에 가기 전에 막혀야 한다: {error}");
        }
    }

    #[test]
    fn the_real_response_shapes_parse() {
        let tags: Vec<TagGroup> = serde_json::from_value(unwrap_data(serde_json::json!({
            "status": true, "message": null,
            "data": [{"category": "dev", "tags": ["Vue", "Nuxt"]}, {"category": "general", "tags": []}]
        })))
        .unwrap();
        assert_eq!(tags[0].tags, ["Vue", "Nuxt"]);

        let guests: GuestList = serde_json::from_value(unwrap_data(serde_json::json!({
            "status": true,
            "data": {"total_guests": 2, "total_attendees": 3, "guests": [
                {"guest_id": 1, "name": "가", "companions": 1, "visit_hour": 12, "created_at": "2026-10-01 03:00:00"},
                {"guest_id": 2, "name": "나", "companions": 0, "visit_hour": null, "created_at": "2026-10-01 04:00:00"}
            ]}
        })))
        .unwrap();
        assert_eq!(guests.guests[1].visit_hour, None);

        let created: CreatedApiKey = serde_json::from_value(unwrap_data(serde_json::json!({
            "status": true, "message": "x",
            "data": {"api_key_id": 5, "name": "cli", "role": "admin", "key": "tyk_a_b", "created_at": "2026-10-15T00:00:00Z"}
        })))
        .unwrap();
        assert_eq!(created.api_key_id, 5);
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
