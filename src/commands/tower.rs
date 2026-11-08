use std::thread::sleep;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::cli::TowerCommand;
use crate::config::Config;
use crate::error::AppError;
use crate::timeutil::{parse_rfc3339, seoul_short};
use crate::tower::{missing_token_message, TokenSource, Tower};

/// 갱신·동기화를 기다릴 때 상태를 다시 보는 간격과 최대 시간.
pub const POLL: Duration = Duration::from_secs(1);
pub const OVERVIEW_WAIT: Duration = Duration::from_secs(180);
pub const SYNC_WAIT: Duration = Duration::from_secs(300);

/// `tower` 서브커맨드 분배. 연결은 한 번 만들고 하위 그룹에 넘긴다.
pub fn run(command: TowerCommand, json: bool, config: &Config) -> Result<(), AppError> {
    let tower = Tower::connect(config)?;
    match command {
        TowerCommand::Status => status(&tower, json),
        TowerCommand::Overview { refresh, source } => overview(&tower, refresh, source.as_deref(), json),
        TowerCommand::Brain { command } => crate::commands::brain::run(&tower, command, json),
        TowerCommand::Netfile { command } => crate::commands::netfile::run(&tower, command, json),
    }
}

/// JSON을 사람이 읽기 좋게 찍는다(`--json` 공통).
pub fn print_json(value: &Value) -> Result<(), AppError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// `fetch`를 `done`이 참이 될 때까지 되풀이한다. 제한 시간이 지나면 에러다.
pub fn wait_for(
    interval: Duration,
    max: Duration,
    what: &str,
    mut fetch: impl FnMut() -> Result<Value, AppError>,
    done: impl Fn(&Value) -> bool,
) -> Result<Value, AppError> {
    let started = Instant::now();
    loop {
        let value = fetch()?;
        if done(&value) {
            return Ok(value);
        }
        if started.elapsed() > max {
            return Err(AppError::RunFailed(format!("{what}이(가) 제한 시간 안에 끝나지 않았습니다")));
        }
        sleep(interval);
    }
}

/// 개요 갱신이 끝났는가(응답의 `refreshing`이 꺼졌는가).
pub fn refresh_done(overview: &Value) -> bool {
    overview["refreshing"] != true
}

/// 바이트를 사람이 읽는 단위로. NetFile 표기(MB = 10의 6제곱)에 맞춘다.
pub fn human_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if bytes < 1000 {
        format!("{bytes} B")
    } else if b < 1e6 {
        format!("{:.1} KB", b / 1e3)
    } else if b < 1e9 {
        format!("{:.1} MB", b / 1e6)
    } else {
        format!("{:.2} GB", b / 1e9)
    }
}

/// tytower가 준 UTC 시각을 한국 시각 `11/8 13:45`로. 읽지 못하면 원문을 그대로 쓴다.
pub fn when(iso: &str) -> String {
    parse_rfc3339(iso).map(seoul_short).unwrap_or_else(|| iso.to_string())
}

// ── tower status ──────────────────────────────────────────────

/// 연결과 토큰을 확인한다. 연결이 안 되면 `TowerNotRunning`, 토큰이 틀리면 에러로 끝난다.
fn status(tower: &Tower, json: bool) -> Result<(), AppError> {
    tower.health()?;
    let token = if !tower.has_token() {
        "missing"
    } else {
        match tower.whoami() {
            Ok(_) => "ok",
            Err(AppError::Api { status: 401, .. }) => "rejected",
            Err(e) => return Err(e),
        }
    };
    let source = match tower.token_source() {
        Some(TokenSource::Config) => Some("config"),
        Some(TokenSource::File(_)) => Some("file"),
        None => None,
    };

    if json {
        print_json(&serde_json::json!({
            "origin": tower.origin(),
            "reachable": true,
            "token": token,
            "token_source": source,
        }))?;
    } else {
        println!("tytower 연결됨: {}", tower.origin());
        let detail = match tower.token_source() {
            Some(TokenSource::Config) => "설정(TYANGE_TOWER_TOKEN)".to_string(),
            Some(TokenSource::File(path)) => format!("토큰 파일 {}", path.display()),
            None => String::new(),
        };
        match token {
            "ok" => println!("토큰: 맞음 ({detail})"),
            "rejected" => println!("토큰: 맞지 않음 ({detail})"),
            _ => println!("토큰: 없음"),
        }
    }

    match token {
        "ok" => Ok(()),
        "rejected" => Err(AppError::Invalid("tytower 토큰이 맞지 않습니다(TYANGE_TOWER_TOKEN이나 토큰 파일을 확인하세요)".into())),
        _ => Err(AppError::Invalid(missing_token_message(tower.is_local()))),
    }
}

// ── tower overview ────────────────────────────────────────────

/// 개요 응답 중 사람이 읽을 부분만. 나머지 필드는 `--json`에서 그대로 나간다.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct OverviewView {
    pub fetched_at: Option<String>,
    pub refreshing: bool,
    pub fixture: bool,
    pub refresh_minutes: u64,
    pub config_note: Option<String>,
    pub work: WorkView,
    pub feeds: Vec<FeedView>,
    pub personal: PersonalView,
    pub craft: Vec<CraftLink>,
}

/// Craft 앱 바로가기 링크(설정 파일의 `[craft]`).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CraftLink {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct WorkView {
    pub status: String,
    pub repo: String,
    pub error: Option<String>,
    pub sample: bool,
    pub reviews: Vec<Item>,
    pub prs: Vec<Item>,
    pub issues: Vec<Item>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct FeedView {
    pub id: String,
    pub label: String,
    pub status: String,
    pub error: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PersonalView {
    pub matches: Vec<Match>,
    pub keywords: Vec<KeywordCount>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Item {
    pub title: String,
    pub url: String,
    pub meta: String,
    pub badge: String,
    pub status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Match {
    pub item: Item,
    pub source_label: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct KeywordCount {
    pub keyword: String,
    pub count: usize,
}

fn overview(tower: &Tower, refresh: bool, source: Option<&str>, json: bool) -> Result<(), AppError> {
    let value = if refresh {
        if !tower.start_overview_refresh()? {
            eprintln!("이미 갱신 중입니다. 끝날 때까지 기다립니다.");
        }
        wait_for(POLL, OVERVIEW_WAIT, "개요 갱신", || tower.overview(), refresh_done)?
    } else {
        tower.overview()?
    };

    if json {
        return print_json(&select_json(&value, source));
    }
    let view: OverviewView = serde_json::from_value(value)?;
    for line in overview_lines(&view, source) {
        println!("{line}");
    }
    Ok(())
}

/// `--source`로 고른 부분만 JSON으로. 없으면 전체 응답 그대로다.
pub fn select_json(value: &Value, source: Option<&str>) -> Value {
    match source {
        None => value.clone(),
        Some("work") => value["work"].clone(),
        Some("personal") => value["personal"].clone(),
        Some(id) => value["feeds"]
            .as_array()
            .and_then(|feeds| feeds.iter().find(|f| f["id"] == id))
            .cloned()
            .unwrap_or(Value::Null),
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "ok" => "정상",
        "stale" => "이전 값 표시(최근 갱신 실패)",
        "error" => "실패",
        "off" => "사용 불가",
        "pending" => "아직 안 받음",
        _ => "알 수 없음",
    }
}

/// 한 항목 두 줄: 제목(배지·설명), 주소.
pub fn item_lines(item: &Item) -> Vec<String> {
    let badge = if item.badge.trim().is_empty() { String::new() } else { format!(" [{}]", item.badge.trim()) };
    let meta = if item.meta.trim().is_empty() { String::new() } else { format!("  {}", item.meta.trim()) };
    vec![format!("  - {}{badge}{meta}", item.title), format!("    {}", item.url)]
}

/// 개요 전체 요약(출처 선택이 없을 때)이나 한 출처의 항목 목록.
pub fn overview_lines(view: &OverviewView, source: Option<&str>) -> Vec<String> {
    match source {
        None => summary_lines(view),
        Some(id) => source_lines(view, id),
    }
}

fn summary_lines(view: &OverviewView) -> Vec<String> {
    let mut lines = Vec::new();
    let mut head = match &view.fetched_at {
        Some(at) => format!("개요 · 마지막 갱신 {} · {}분마다", when(at), view.refresh_minutes),
        None => "개요 · 아직 첫 갱신 전입니다".to_string(),
    };
    if view.fixture {
        head.push_str(" · 고정 데이터(TYTOWER_OVERVIEW_FIXTURE)");
    }
    if view.refreshing {
        head.push_str(" · 갱신 중");
    }
    lines.push(head);
    if let Some(note) = &view.config_note {
        lines.push(format!("설정 파일 주의: {note}"));
    }

    let work = &view.work;
    let mut work_line = format!(
        "Work · {} · {}",
        work.repo,
        status_label(&work.status)
    );
    if work.sample {
        work_line.push_str(" · 샘플");
    }
    if work.status != "pending" {
        work_line.push_str(&format!(
            " · 리뷰 {} · 내 PR {} · 이슈 {}",
            work.reviews.len(),
            work.prs.len(),
            work.issues.len()
        ));
    }
    lines.push(work_line);
    if let Some(error) = &work.error {
        lines.push(format!("  └ {error}"));
    }

    for feed in &view.feeds {
        let mut line = format!("{} · {} · {}건", feed.label, status_label(&feed.status), feed.items.len());
        if let Some(error) = &feed.error {
            line.push_str(&format!(" — {error}"));
        }
        lines.push(line);
    }

    let matched = view.personal.matches.len();
    let by_keyword = view
        .personal
        .keywords
        .iter()
        .map(|k| format!("{} {}", k.keyword, k.count))
        .collect::<Vec<_>>()
        .join(" · ");
    if by_keyword.is_empty() {
        lines.push(format!("관심사 소식 · {matched}건"));
    } else {
        lines.push(format!("관심사 소식 · {matched}건 ({by_keyword})"));
    }
    for link in &view.craft {
        lines.push(format!("Craft · {} · {}", link.title, link.url));
    }
    lines
}

fn source_lines(view: &OverviewView, id: &str) -> Vec<String> {
    let mut lines = Vec::new();
    match id {
        "work" => {
            lines.push(format!("Work · {} · {}", view.work.repo, status_label(&view.work.status)));
            if let Some(error) = &view.work.error {
                lines.push(format!("  └ {error}"));
            }
            for (title, items) in [
                ("리뷰 요청", &view.work.reviews),
                ("내 PR", &view.work.prs),
                ("할당된 이슈", &view.work.issues),
            ] {
                lines.push(format!("{title} ({})", items.len()));
                for item in items {
                    lines.extend(item_lines(item));
                }
            }
        }
        "personal" => {
            lines.push(format!("관심사 소식 ({}건)", view.personal.matches.len()));
            for m in &view.personal.matches {
                lines.push(format!("  [{}] {}", m.source_label, m.keywords.join(", ")));
                lines.extend(item_lines(&m.item));
            }
        }
        feed_id => match view.feeds.iter().find(|f| f.id == feed_id) {
            Some(feed) => {
                lines.push(format!("{} · {} · {}건", feed.label, status_label(&feed.status), feed.items.len()));
                if let Some(error) = &feed.error {
                    lines.push(format!("  └ {error}"));
                }
                for item in &feed.items {
                    lines.extend(item_lines(item));
                }
            }
            None => lines.push(format!("그런 출처가 없습니다: {feed_id}")),
        },
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 실제 응답 모양을 줄인 것(docs/api-overview.md의 예와 같은 키).
    fn fixture() -> Value {
        json!({
            "fetched_at": "2026-11-08T04:45:31Z",
            "refreshing": false,
            "fixture": true,
            "refresh_minutes": 30,
            "config_path": "C:\\x\\config.toml",
            "config_note": null,
            "config": {"work_repo": "abc-us/abc-web-apps", "github_user": "tyange", "site_base_url": "https://tyange.com", "interests": []},
            "work": {
                "status": "ok", "repo": "abc-us/abc-web-apps", "error": null, "fetched_at": "2026-11-08T04:45:31Z",
                "sample": true,
                "reviews": [{"title": "리뷰 A", "url": "https://x/1", "meta": "owner", "badge": "", "status": "open", "value": 0, "at": null}],
                "prs": [], "issues": []
            },
            "feeds": [
                {"id": "geeknews", "label": "GeekNews", "status": "ok", "error": null, "url": null, "fetched_at": "2026-11-08T04:45:31Z",
                 "items": [{"title": "Rust 소식", "url": "https://gn/1", "meta": "11/8", "badge": "점수 42", "status": null, "value": 42, "at": null}]},
                {"id": "hn", "label": "Hacker News", "status": "stale", "error": "timeout", "url": null, "fetched_at": null,
                 "items": []},
                {"id": "trending", "label": "GitHub 새 인기 저장소", "status": "pending", "error": null, "url": null, "fetched_at": null, "items": []},
                {"id": "tyange", "label": "tyange.com", "status": "error", "error": "https://tyange.com/index.xml: 응답이 없어요", "url": null, "fetched_at": null, "items": []},
                {"id": "repos", "label": "내 GitHub 저장소", "status": "ok", "error": null, "url": null, "fetched_at": null, "items": []}
            ],
            "personal": {
                "matches": [{"item": {"title": "Rust 소식", "url": "https://gn/1", "meta": "11/8", "badge": "", "status": null, "value": 0, "at": null},
                             "source_id": "geeknews", "source_label": "GeekNews", "keywords": ["rust"]}],
                "keywords": [{"keyword": "rust", "area": "work", "count": 1}],
                "bubbles": []
            },
            "craft": [{"title": "홈 대시보드", "url": "craftdocs://open?x=1"}]
        })
    }

    fn view() -> OverviewView {
        serde_json::from_value(fixture()).unwrap()
    }

    #[test]
    fn the_summary_names_every_source_with_its_state() {
        let lines = overview_lines(&view(), None);
        assert!(lines[0].contains("2026-11-08") || lines[0].contains("11/8"), "{lines:?}");
        assert!(lines[0].contains("고정 데이터"), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("GeekNews · 정상 · 1건")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Hacker News · 이전 값 표시")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("tyange.com · 실패 · 0건 — https://tyange.com")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("Work · abc-us/abc-web-apps · 정상 · 샘플")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("관심사 소식 · 1건 (rust 1)")), "{lines:?}");
        assert!(lines.iter().any(|l| l == "Craft · 홈 대시보드 · craftdocs://open?x=1"), "{lines:?}");
    }

    #[test]
    fn one_source_lists_its_items_with_their_links() {
        let lines = overview_lines(&view(), Some("geeknews"));
        assert_eq!(lines[0], "GeekNews · 정상 · 1건");
        assert_eq!(lines[1], "  - Rust 소식 [점수 42]  11/8");
        assert_eq!(lines[2], "    https://gn/1");
        let work = overview_lines(&view(), Some("work"));
        assert!(work.contains(&"리뷰 요청 (1)".to_string()), "{work:?}");
        let personal = overview_lines(&view(), Some("personal"));
        assert!(personal.iter().any(|l| l.contains("[GeekNews] rust")), "{personal:?}");
    }

    #[test]
    fn a_pending_overview_says_it_has_not_been_fetched_yet() {
        let pending = json!({
            "fetched_at": null, "refreshing": true, "fixture": false, "refresh_minutes": 30,
            "work": {"status": "pending", "repo": "r/r", "reviews": [], "prs": [], "issues": []},
            "feeds": [], "personal": {"matches": [], "keywords": []}
        });
        let view: OverviewView = serde_json::from_value(pending).unwrap();
        let lines = overview_lines(&view, None);
        assert!(lines[0].contains("첫 갱신 전") && lines[0].contains("갱신 중"), "{lines:?}");
        assert!(lines.iter().any(|l| l == "Work · r/r · 아직 안 받음"), "{lines:?}");
    }

    #[test]
    fn json_passes_the_answer_through_and_a_source_selects_its_part() {
        let value = fixture();
        assert_eq!(select_json(&value, None), value);
        assert_eq!(select_json(&value, Some("hn"))["id"], "hn");
        assert_eq!(select_json(&value, Some("work"))["repo"], "abc-us/abc-web-apps");
        assert_eq!(select_json(&value, Some("personal"))["keywords"][0]["keyword"], "rust");
        assert!(select_json(&value, Some("nope")).is_null());
    }

    #[test]
    fn a_refresh_is_done_when_the_overview_stops_saying_refreshing() {
        assert!(refresh_done(&json!({"refreshing": false})));
        assert!(refresh_done(&json!({})), "a missing flag means not refreshing");
        assert!(!refresh_done(&json!({"refreshing": true})));
    }

    #[test]
    fn human_bytes_use_the_netfile_units() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_500), "1.5 KB");
        assert_eq!(human_bytes(12_000_000), "12.0 MB");
        assert_eq!(human_bytes(2_500_000_000), "2.50 GB");
    }

    #[test]
    fn wait_for_returns_once_done_and_gives_up_after_the_limit() {
        let mut calls = 0;
        let value = wait_for(
            Duration::ZERO,
            Duration::from_secs(5),
            "테스트",
            || {
                calls += 1;
                Ok(json!({"status": if calls < 3 { "running" } else { "done" }}))
            },
            |v| v["status"] != "running",
        )
        .unwrap();
        assert_eq!(value["status"], "done");
        assert_eq!(calls, 3);

        let error = wait_for(Duration::ZERO, Duration::ZERO, "테스트", || Ok(json!({"status": "running"})), |_| false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("제한 시간"), "{error}");
    }
}
