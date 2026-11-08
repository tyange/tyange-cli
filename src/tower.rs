//! tytower(데스크톱 컨트롤 타워)의 로컬 HTTP API 클라이언트. `tower` 명령은 이 층만 거친다.
//!
//! 로직은 여기 없다. tytower가 하는 일을 다시 만들지 않고 HTTP로 부른 결과를 보여 줄 뿐이다.
//! 토큰은 `Authorization: Bearer <토큰>`으로만 나가고, 출력이나 에러 문구에는 들어가지 않는다
//! (`Debug`도 값을 가린다). 응답 본문은 서버가 준 그대로 `serde_json::Value`로 들고 다니고,
//! `--json`은 그 값을 그대로 찍는다.

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Method, Url};
use serde_json::{json, Value};

use crate::config::Config;
use crate::error::AppError;

/// tytower가 토큰 파일을 두는 앱 식별자(`tauri.conf.json`의 identifier와 같다).
pub const APP_IDENTIFIER: &str = "com.tyange.tytower";
/// 토큰 파일 이름(tytower의 `auth.rs`와 같다).
pub const TOKEN_FILE_NAME: &str = "tower-token";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// 토큰을 어디서 얻었는가. 출력에는 "설정"인지 "파일"인지와 파일 경로만 나간다(값은 없다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// 환경 변수나 env 파일(`TYANGE_TOWER_TOKEN`)
    Config,
    /// tytower가 만든 토큰 파일(이 PC의 localhost에서만 읽는다)
    File(PathBuf),
}

/// 서버가 돌려준 응답 한 개. 본문은 아직 해석하지 않았다.
pub struct Reply {
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub struct Tower {
    http: Client,
    /// 끝 슬래시가 없는 기본 주소. 요청 경로(`/api/...`)를 붙인다.
    base: String,
    /// `http://호스트:포트`. 에러와 상태 출력에 쓴다(경로·사용자 정보는 뺀다).
    origin: String,
    local: bool,
    token: Option<String>,
    token_source: Option<TokenSource>,
}

impl std::fmt::Debug for Tower {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tower")
            .field("origin", &self.origin)
            .field("token", &if self.token.is_some() { "<설정됨>" } else { "<없음>" })
            .finish()
    }
}

impl Tower {
    /// 설정에서 주소와 토큰을 읽어 연결 객체를 만든다. 토큰이 없어도 만들어진다(상태 확인은 토큰 없이도 된다).
    /// 토큰이 필요한 호출은 보내기 전에 토큰이 없다고 알려 준다.
    pub fn connect(config: &Config) -> Result<Tower, AppError> {
        let url = Url::parse(config.tower_url()).map_err(|_| {
            AppError::Invalid("TYANGE_TOWER_URL을 읽지 못했습니다. http://127.0.0.1:47820 같은 주소여야 합니다".into())
        })?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::Invalid("TYANGE_TOWER_URL은 http:// 또는 https:// 주소여야 합니다".into()));
        }
        let local = is_local(&url);
        let path = token_path(config.tower_config_dir(), std::env::consts::OS, |key| std::env::var(key).ok());
        let found = resolve_token(config.tower_token(), local, path.as_deref(), |p| std::fs::read_to_string(p));
        let (token, token_source) = match found {
            Some((token, source)) => (Some(token), Some(source)),
            None => (None, None),
        };
        Self::build(config.tower_url(), &url, local, token, token_source)
    }

    /// 주소와 토큰을 직접 넣어 만든다(테스트용). 파일이나 설정을 읽지 않는다.
    #[cfg(test)]
    pub fn with_token(base: &str, token: Option<&str>) -> Result<Tower, AppError> {
        let url = Url::parse(base).map_err(|_| AppError::Invalid("tytower 주소가 잘못됐습니다".into()))?;
        let local = is_local(&url);
        let source = token.map(|_| TokenSource::Config);
        Self::build(base, &url, local, token.map(str::to_string), source)
    }

    fn build(
        base: &str,
        url: &Url,
        local: bool,
        token: Option<String>,
        token_source: Option<TokenSource>,
    ) -> Result<Tower, AppError> {
        let mut builder = Client::builder().connect_timeout(CONNECT_TIMEOUT).timeout(REQUEST_TIMEOUT);
        if local {
            // 프록시 설정(HTTP_PROXY 등)이 이 PC의 tytower로 가는 요청을 가로채지 않게 한다.
            builder = builder.no_proxy();
        }
        Ok(Tower {
            http: builder.build()?,
            base: base.trim_end_matches('/').to_string(),
            origin: url.origin().ascii_serialization(),
            local,
            token,
            token_source,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn token_source(&self) -> Option<&TokenSource> {
        self.token_source.as_ref()
    }

    fn token(&self) -> Result<&str, AppError> {
        self.token.as_deref().ok_or_else(|| AppError::Invalid(missing_token_message(self.local)))
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
        auth: bool,
    ) -> Result<Reply, AppError> {
        let mut req: RequestBuilder = self.http.request(method, format!("{}{path}", self.base)).query(query);
        if auth {
            req = req.header(AUTHORIZATION, format!("Bearer {}", self.token()?));
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        let response = req.send().map_err(|e| self.transport(e))?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = response.bytes().map_err(|e| self.transport(e))?.to_vec();
        Ok(Reply { status, content_type, bytes })
    }

    fn transport(&self, e: reqwest::Error) -> AppError {
        if e.is_connect() {
            AppError::TowerNotRunning(self.origin.clone())
        } else if e.is_timeout() {
            AppError::Network("tytower가 제때 응답하지 않았습니다(시간 초과)".into())
        } else {
            AppError::from(e)
        }
    }

    fn get(&self, path: &str) -> Result<Value, AppError> {
        parse_reply(self.request(Method::GET, path, &[], None, true)?)
    }

    fn post(&self, path: &str, body: Option<&Value>) -> Result<Value, AppError> {
        parse_reply(self.request(Method::POST, path, &[], body, true)?)
    }

    fn delete(&self, path: &str) -> Result<Value, AppError> {
        parse_reply(self.request(Method::DELETE, path, &[], None, true)?)
    }

    /// 시작 계열 호출(갱신·동기화): 202면 새로 시작했다(`true`), 409면 이미 돌고 있다(`false`).
    fn start(&self, path: &str) -> Result<bool, AppError> {
        let reply = self.request(Method::POST, path, &[], None, true)?;
        if reply.status == 409 {
            return Ok(false);
        }
        parse_reply(reply)?;
        Ok(true)
    }

    // ── 연결 확인 ──────────────────────────────────────────────

    /// `GET /api/health`(토큰 없이 된다). 연결이 안 되면 `TowerNotRunning`.
    pub fn health(&self) -> Result<Value, AppError> {
        parse_reply(self.request(Method::GET, "/api/health", &[], None, false)?)
    }

    /// `GET /api/whoami`. 토큰이 맞으면 `{"ok": true}`, 틀리면 401.
    pub fn whoami(&self) -> Result<Value, AppError> {
        self.get("/api/whoami")
    }

    // ── 개요 ───────────────────────────────────────────────────

    pub fn overview(&self) -> Result<Value, AppError> {
        self.get("/api/overview")
    }

    /// 갱신을 시작한다. 새로 시작했으면 `true`, 이미 돌고 있었으면 `false`.
    pub fn start_overview_refresh(&self) -> Result<bool, AppError> {
        self.start("/api/overview/refresh")
    }

    // ── 브레인 ─────────────────────────────────────────────────

    pub fn brain_index(&self) -> Result<Value, AppError> {
        self.get("/api/brain/notes")
    }

    pub fn brain_note(&self, id: &str) -> Result<Value, AppError> {
        self.get(&format!("/api/brain/notes/{}", check_id(id)?))
    }

    pub fn brain_search(&self, query: &str, limit: u64) -> Result<Value, AppError> {
        let limit = limit.to_string();
        parse_reply(self.request(
            Method::GET,
            "/api/brain/search",
            &[("q", query), ("limit", &limit)],
            None,
            true,
        )?)
    }

    /// 노트 안의 이미지 파일. 내용 타입과 바이트를 돌려준다.
    pub fn brain_file(&self, id: &str) -> Result<(Option<String>, Vec<u8>), AppError> {
        let reply = self.request(Method::GET, &format!("/api/brain/files/{}", check_id(id)?), &[], None, true)?;
        if !(200..300).contains(&reply.status) {
            return Err(tower_error(reply.status, &String::from_utf8_lossy(&reply.bytes)));
        }
        Ok((reply.content_type, reply.bytes))
    }

    pub fn brain_sync_status(&self) -> Result<Value, AppError> {
        self.get("/api/brain/sync")
    }

    /// 동기화를 시작한다. 새로 시작했으면 `true`, 이미 돌고 있었으면 `false`.
    pub fn start_brain_sync(&self) -> Result<bool, AppError> {
        self.start("/api/brain/sync")
    }

    // ── NetFile ────────────────────────────────────────────────

    pub fn netfile_status(&self) -> Result<Value, AppError> {
        self.get("/api/netfile/status")
    }

    pub fn netfile_search(&self, body: &Value) -> Result<Value, AppError> {
        self.post("/api/netfile/search", Some(body))
    }

    pub fn netfile_ls(&self, folder_id: &str) -> Result<Value, AppError> {
        self.post("/api/netfile/ls", Some(&json!({ "folder_id": check_id(folder_id)? })))
    }

    pub fn netfile_jobs(&self) -> Result<Value, AppError> {
        self.get("/api/netfile/downloads")
    }

    pub fn netfile_start(&self, entry_ids: &[String]) -> Result<Value, AppError> {
        for id in entry_ids {
            check_id(id)?;
        }
        self.post("/api/netfile/downloads", Some(&json!({ "entry_ids": entry_ids })))
    }

    /// 완료된 작업의 파일을 지운다(서버가 다운로드 폴더 안의 일반 파일만 지운다).
    pub fn netfile_delete_file(&self, job_id: &str) -> Result<Value, AppError> {
        self.delete(&format!("/api/netfile/downloads/{}/file", check_id(job_id)?))
    }

    /// 작업을 취소하거나(받는 중·대기 중) 끝난 작업을 목록에서 지운다.
    pub fn netfile_cancel(&self, job_id: &str) -> Result<Value, AppError> {
        self.delete(&format!("/api/netfile/downloads/{}", check_id(job_id)?))
    }
}

/// 서버 응답을 값으로. 2xx가 아니면 서버의 에러 모양을 해석한다. 빈 본문은 `null`이다.
pub fn parse_reply(reply: Reply) -> Result<Value, AppError> {
    let text = String::from_utf8_lossy(&reply.bytes);
    if !(200..300).contains(&reply.status) {
        return Err(tower_error(reply.status, &text));
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_str(&text)?)
}

/// 실패한 응답을 에러로. 401은 토큰 문제로 안내하고, 나머지는 `{"error","message"}`의 문구를 쓴다.
pub fn tower_error(status: u16, body: &str) -> AppError {
    if status == 401 {
        return AppError::Api {
            status,
            message: "tytower 토큰이 맞지 않습니다(TYANGE_TOWER_TOKEN이나 토큰 파일을 확인하세요)".into(),
        };
    }
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|v| v.get("message").and_then(Value::as_str))
        .map(str::to_string);
    let code = parsed
        .as_ref()
        .and_then(|v| v.get("error").and_then(Value::as_str))
        .unwrap_or("");
    let message = message.unwrap_or_else(|| match code {
        "already_running" => "이미 실행 중입니다. 끝난 뒤 다시 하세요".to_string(),
        "invalid_id" => "잘못된 id입니다".to_string(),
        "not_found" => "찾지 못했습니다".to_string(),
        "" => body.chars().take(200).collect(),
        other => other.to_string(),
    });
    AppError::Api { status, message }
}

/// 토큰이 없을 때의 안내. 원격 주소에서는 토큰 파일을 읽지 않는다는 것도 알려 준다.
pub fn missing_token_message(local: bool) -> String {
    if local {
        "tytower 토큰이 없습니다. TYANGE_TOWER_TOKEN을 설정하거나, 이 PC에서 tytower를 한 번 실행해 토큰 파일을 만드세요".into()
    } else {
        "원격 tytower에는 토큰 파일을 쓰지 않습니다. TYANGE_TOWER_TOKEN을 설정하세요".into()
    }
}

/// 주소가 이 PC(localhost)를 가리키는가. 토큰 파일은 이때만 읽는다.
pub fn is_local(url: &Url) -> bool {
    matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
}

/// 토큰 파일 위치. `TYTOWER_CONFIG_DIR`가 있으면 그 폴더, 없으면 tytower가 쓰는 앱 설정 폴더다.
/// `os`와 `lookup`(환경 변수 조회)을 받아서 어느 OS에서든 테스트할 수 있다.
pub fn token_path(config_dir: Option<&str>, os: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(dir) = config_dir.filter(|d| !d.trim().is_empty()) {
        return Some(PathBuf::from(dir).join(TOKEN_FILE_NAME));
    }
    let base = match os {
        "windows" => PathBuf::from(lookup("APPDATA")?),
        "macos" => PathBuf::from(lookup("HOME")?).join("Library").join("Application Support"),
        _ => match lookup("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            Some(xdg) => PathBuf::from(xdg),
            None => PathBuf::from(lookup("HOME")?).join(".config"),
        },
    };
    Some(base.join(APP_IDENTIFIER).join(TOKEN_FILE_NAME))
}

/// 토큰을 고른다. 설정이 최우선이다. 설정이 없고 localhost일 때만 토큰 파일을 읽는다.
/// 파일이 없거나 비어 있으면 `None`(없다고 안내한다).
pub fn resolve_token(
    config_token: Option<&str>,
    local: bool,
    path: Option<&Path>,
    read: impl Fn(&Path) -> std::io::Result<String>,
) -> Option<(String, TokenSource)> {
    if let Some(token) = config_token {
        return Some((token.to_string(), TokenSource::Config));
    }
    if !local {
        return None;
    }
    let path = path?;
    let text = read(path).ok()?;
    let token = text.trim();
    if token.is_empty() {
        return None;
    }
    Some((token.to_string(), TokenSource::File(path.to_path_buf())))
}

/// 경로에 넣어도 안전한 id인가(영문·숫자·`-`·`_`, 1~200자). 브레인 노트 id와 NetFile id가 이 모양이다.
pub fn check_id(id: &str) -> Result<&str, AppError> {
    let ok = !id.is_empty()
        && id.chars().count() <= 200
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(id)
    } else {
        Err(AppError::Invalid(format!("id 모양이 맞지 않습니다: {}", id.chars().take(40).collect::<String>())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const TOKEN: &str = "tower-token-secret-0123456789abcdef";

    fn no_file(_: &Path) -> std::io::Result<String> {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"))
    }

    #[test]
    fn the_configured_token_beats_the_file() {
        let path = PathBuf::from("/x/tower-token");
        let found = resolve_token(Some(TOKEN), true, Some(&path), no_file).unwrap();
        assert_eq!(found, (TOKEN.to_string(), TokenSource::Config));
    }

    #[test]
    fn the_token_file_is_read_only_for_localhost_and_is_trimmed() {
        let path = PathBuf::from("/x/tower-token");
        let read = |_: &Path| Ok(format!("{TOKEN}\n"));
        let found = resolve_token(None, true, Some(&path), read).unwrap();
        assert_eq!(found.0, TOKEN);
        assert_eq!(found.1, TokenSource::File(path.clone()));

        // 원격 주소에서는 파일을 아예 읽지 않는다.
        let touched = Cell::new(false);
        let spy = |_: &Path| {
            touched.set(true);
            Ok(TOKEN.to_string())
        };
        assert!(resolve_token(None, false, Some(&path), spy).is_none());
        assert!(!touched.get(), "remote addresses must never read the local token file");
    }

    #[test]
    fn an_empty_or_missing_token_file_means_no_token() {
        let path = PathBuf::from("/x/tower-token");
        assert!(resolve_token(None, true, Some(&path), |_| Ok("  \n".into())).is_none());
        assert!(resolve_token(None, true, Some(&path), no_file).is_none());
        assert!(resolve_token(None, true, None, no_file).is_none());
    }

    #[test]
    fn the_token_file_location_follows_tytowers_rules() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
        };
        // TYTOWER_CONFIG_DIR가 있으면 그 폴더
        assert_eq!(
            token_path(Some("/tmp/cfg"), "windows", env(&[])),
            Some(PathBuf::from("/tmp/cfg").join("tower-token"))
        );
        // Windows: %APPDATA%\com.tyange.tytower\tower-token
        let win = token_path(None, "windows", env(&[("APPDATA", "C:\\Users\\me\\AppData\\Roaming")])).unwrap();
        assert!(win.ends_with("com.tyange.tytower/tower-token") || win.ends_with("com.tyange.tytower\\tower-token"));
        assert!(win.starts_with("C:\\Users\\me\\AppData\\Roaming"));
        // macOS
        let mac = token_path(None, "macos", env(&[("HOME", "/Users/me")])).unwrap();
        assert_eq!(mac, PathBuf::from("/Users/me/Library/Application Support/com.tyange.tytower/tower-token"));
        // Linux: XDG_CONFIG_HOME이 있으면 그것, 없으면 ~/.config
        let xdg = token_path(None, "linux", env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")])).unwrap();
        assert_eq!(xdg, PathBuf::from("/x/com.tyange.tytower/tower-token"));
        let home = token_path(None, "linux", env(&[("HOME", "/h")])).unwrap();
        assert_eq!(home, PathBuf::from("/h/.config/com.tyange.tytower/tower-token"));
        // 필요한 환경 변수가 없으면 모른다
        assert!(token_path(None, "windows", env(&[])).is_none());
    }

    #[test]
    fn only_localhost_names_count_as_local() {
        for (url, local) in [
            ("http://127.0.0.1:47820", true),
            ("http://localhost:47820", true),
            ("http://[::1]:47820", true),
            ("http://tower.tailnet.example:47820", false),
            ("http://100.64.0.5:47820", false),
        ] {
            assert_eq!(is_local(&Url::parse(url).unwrap()), local, "{url}");
        }
    }

    #[test]
    fn ids_must_be_path_safe() {
        assert!(check_id("MDMvIOyekOujjC/uucu.md").is_err(), "a slash is not an id character");
        assert!(check_id("abc_DEF-123").is_ok());
        assert!(check_id("").is_err());
        assert!(check_id("../secret").is_err());
        assert!(check_id(&"a".repeat(201)).is_err());
    }

    #[test]
    fn a_401_is_explained_as_a_token_problem_and_never_echoes_the_body() {
        match tower_error(401, r#"{"error":"unauthorized"}"#) {
            AppError::Api { status: 401, message } => {
                assert!(message.contains("토큰"), "{message}");
                assert!(!message.contains("unauthorized"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn other_errors_use_the_servers_message_or_a_plain_reading_of_the_code() {
        match tower_error(409, r#"{"error":"already_running"}"#) {
            AppError::Api { status: 409, message } => assert!(message.contains("이미 실행 중")),
            other => panic!("{other:?}"),
        }
        match tower_error(409, r#"{"error":"no_credentials","message":"로그인 값이 없어요"}"#) {
            AppError::Api { status: 409, message } => assert_eq!(message, "로그인 값이 없어요"),
            other => panic!("{other:?}"),
        }
        match tower_error(502, "<html>bad gateway</html>") {
            AppError::Api { status: 502, message } => assert!(message.starts_with("<html>")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn success_with_an_empty_body_is_null_and_a_bad_body_is_an_error() {
        assert_eq!(parse_reply(Reply { status: 200, content_type: None, bytes: vec![] }).unwrap(), Value::Null);
        assert!(parse_reply(Reply { status: 200, content_type: None, bytes: b"{not json".to_vec() }).is_err());
    }

    #[test]
    fn debug_never_shows_the_token_or_the_path_part_of_the_url() {
        let tower = Tower::with_token("http://user:pw@127.0.0.1:47820/some/path/", Some(TOKEN)).unwrap();
        let shown = format!("{tower:?}");
        assert!(!shown.contains(TOKEN), "{shown}");
        assert!(!shown.contains("pw") && !shown.contains("some/path"), "{shown}");
        assert_eq!(tower.origin(), "http://127.0.0.1:47820");
    }

    #[test]
    fn a_missing_token_is_reported_before_any_request() {
        let tower = Tower::with_token("http://127.0.0.1:1", None).unwrap();
        let error = tower.netfile_jobs().unwrap_err().to_string();
        assert!(error.contains("TYANGE_TOWER_TOKEN"), "{error}");
    }
}
