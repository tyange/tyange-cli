//! `tyange tower …` 통합 테스트. 흉내 낸 tytower(mock.rs)에 실제 바이너리를 붙여서 확인한다.
//! 확인하는 것: 성공 출력, `--json` 출력, `Authorization: Bearer` 헤더, 토큰이 출력에 없음,
//! tytower가 꺼져 있을 때의 안내, `rm`이 `--yes` 없이는 지우지 않음.

mod support;

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};
use support::{closed_port_url, scratch, tyange, Mock, Reply, Req, Run};

const TOKEN: &str = "tower-test-token-SECRET-0123456789abcdef";
const AUTH: &str = "Bearer tower-test-token-SECRET-0123456789abcdef";

// ── 흉내 낸 tytower의 응답 ─────────────────────────────────────

fn overview() -> Value {
    json!({
        "fetched_at": "2026-11-08T04:45:31Z",
        "refreshing": false,
        "fixture": false,
        "refresh_minutes": 30,
        "config_path": "C:\\Users\\me\\AppData\\Roaming\\tyradar\\config.toml",
        "config_note": null,
        "config": {"work_repo": "abc-us/abc-web-apps", "github_user": "tyange", "site_base_url": "https://tyange.com", "interests": []},
        "work": {
            "status": "ok", "repo": "abc-us/abc-web-apps", "error": null, "fetched_at": "2026-11-08T04:45:31Z",
            "sample": false,
            "reviews": [{"title": "리뷰할 PR", "url": "https://github.com/abc-us/abc-web-apps/pull/1", "meta": "kim", "badge": "", "status": "open", "value": 0, "at": "2026-11-08T03:00:00Z"}],
            "prs": [], "issues": []
        },
        "feeds": [
            {"id": "geeknews", "label": "GeekNews", "status": "ok", "error": null, "url": null, "fetched_at": "2026-11-08T04:45:31Z",
             "items": [{"title": "Rust 소식", "url": "https://news.hada.io/topic?id=1", "meta": "11/8", "badge": "점수 42", "status": null, "value": 42, "at": "2026-11-08T04:00:00Z"}]},
            {"id": "hn", "label": "Hacker News", "status": "ok", "error": null, "url": null, "fetched_at": "2026-11-08T04:45:31Z",
             "items": [{"title": "Show HN: 무언가", "url": "https://example.com/hn", "meta": "1시간 전", "badge": "점수 9", "status": null, "value": 9, "at": null}]},
            {"id": "trending", "label": "GitHub 새 인기 저장소", "status": "pending", "error": null, "url": null, "fetched_at": null, "items": []},
            {"id": "tyange", "label": "tyange.com", "status": "error", "error": "https://tyange.com/index.xml: 응답이 없어요", "url": null, "fetched_at": null, "items": []},
            {"id": "repos", "label": "내 GitHub 저장소", "status": "ok", "error": null, "url": null, "fetched_at": null, "items": []}
        ],
        "personal": {
            "matches": [{"item": {"title": "Rust 소식", "url": "https://news.hada.io/topic?id=1", "meta": "11/8", "badge": "", "status": null, "value": 0, "at": null},
                         "source_id": "geeknews", "source_label": "GeekNews", "keywords": ["rust"]}],
            "keywords": [{"keyword": "rust", "area": "work", "count": 1}],
            "bubbles": []
        },
        "craft": [{"title": "홈 대시보드", "url": "craftdocs://open?x=1"}]
    })
}

const NOTE_A: &str = "NjAxLmpzb24"; // "03 자료/글.md"
const NOTE_B: &str = "c2Vjb25k"; // "두 번째.md"

fn index() -> Value {
    json!({
        "available": true, "root_name": "brain", "count": 2,
        "tree": {"name": "", "path": "", "count": 2, "folders": [], "notes": [NOTE_B]},
        "notes": [
            {"id": NOTE_A, "title": "글", "folder": "03 자료", "path": "03 자료/글.md", "modified": "2026-11-08T04:09:40Z"},
            {"id": NOTE_B, "title": "두 번째", "folder": "", "path": "두 번째.md", "modified": null}
        ]
    })
}

fn note_a() -> Value {
    json!({
        "id": NOTE_A, "title": "글", "folder": "03 자료", "path": "03 자료/글.md", "modified": "2026-11-08T04:09:40Z",
        "markdown": "# 제목\n\n본문 rust 이야기",
        "links": [
            {"href": "두 번째", "kind": "note", "id": NOTE_B},
            {"href": "pic.png", "kind": "file", "id": "ZmlsZS1pZA"},
            {"href": "https://example.com", "kind": "external", "id": null}
        ],
        "backlinks": [{"id": NOTE_B, "title": "두 번째", "folder": ""}]
    })
}

fn note_b() -> Value {
    json!({
        "id": NOTE_B, "title": "두 번째", "folder": "", "path": "두 번째.md", "modified": null,
        "markdown": "두 번째 노트", "links": [], "backlinks": []
    })
}

fn search() -> Value {
    json!({
        "q": "rust", "available": true,
        "hits": [{"id": NOTE_A, "title": "글", "folder": "03 자료", "in_title": false, "snippet": "본문 rust 이야기"}]
    })
}

fn sync_done() -> Value {
    json!({
        "status": "done", "remote": "/srv/brain.git", "auto_sync": false,
        "started_at": "2026-11-08T04:50:00Z", "finished_at": "2026-11-08T04:50:03Z",
        "report": {"committed": true, "pulled": false, "pushed": true, "cloned": false, "conflict_copies": []},
        "error": null
    })
}

fn sync_running() -> Value {
    json!({"status": "running", "remote": "/srv/brain.git", "auto_sync": false, "started_at": "2026-11-08T04:50:00Z", "finished_at": null, "report": null, "error": null})
}

fn netfile_status() -> Value {
    json!({
        "credentials": {"state": "ok", "source": "environment"},
        "download_dir": "C:\\Users\\me\\Downloads\\tytower-netfile",
        "running": 1, "queued": 0, "max_concurrent": 2
    })
}

fn listing() -> Value {
    json!({
        "total": 2, "more": false,
        "entries": [
            {"id": "f1", "kind": "folder", "name": "회의", "size": ""},
            {"id": "x9", "kind": "file", "name": "회의록.docx", "size": "120 KB"}
        ]
    })
}

fn job_done() -> Value {
    json!({"id": "job-done", "entry_id": "e1", "name": "영상.mp4", "file_name": "영상.mp4", "state": "done",
           "bytes_done": 3400000, "total": 3400000, "bytes_per_sec": 0.0, "message": null, "created_ms": 1})
}

fn job_running() -> Value {
    json!({"id": "job-run", "entry_id": "e2", "name": "자료.pdf", "file_name": "자료.pdf", "state": "running",
           "bytes_done": 5000000, "total": 20000000, "bytes_per_sec": 1.5, "message": null, "created_ms": 2})
}

fn job_new() -> Value {
    json!({"id": "job-new", "entry_id": "e1", "name": "영상.mp4", "file_name": "영상.mp4", "state": "queued",
           "bytes_done": 0, "total": null, "bytes_per_sec": 0.0, "message": null, "created_ms": 3})
}

/// 모든 경로를 흉내 낸다. `/api/health`만 토큰 없이 되고, 나머지는 토큰이 맞아야 한다(tytower와 같다).
fn full_route(req: &Req) -> Reply {
    if req.path() == "/api/health" {
        return Reply::json(200, json!({"status": "ok"}));
    }
    if req.auth.as_deref() != Some(AUTH) {
        return Reply::json(401, json!({"error": "unauthorized"}));
    }
    let path = req.path();
    match (req.method.as_str(), path) {
        ("GET", "/api/whoami") => Reply::json(200, json!({"ok": true})),
        ("GET", "/api/overview") => Reply::json(200, overview()),
        ("POST", "/api/overview/refresh") => Reply::json(202, json!({"status": "started"})),
        ("GET", "/api/brain/notes") => Reply::json(200, index()),
        ("GET", "/api/brain/search") => Reply::json(200, search()),
        ("GET", "/api/brain/sync") => Reply::json(200, sync_done()),
        ("POST", "/api/brain/sync") => Reply::json(202, sync_running()),
        ("GET", p) if p == format!("/api/brain/notes/{NOTE_A}") => Reply::json(200, note_a()),
        ("GET", p) if p == format!("/api/brain/notes/{NOTE_B}") => Reply::json(200, note_b()),
        ("GET", p) if p.starts_with("/api/brain/files/") => Reply::bytes(200, "image/png", b"PNGDATA"),
        ("GET", "/api/netfile/status") => Reply::json(200, netfile_status()),
        ("POST", "/api/netfile/search") | ("POST", "/api/netfile/ls") => Reply::json(200, listing()),
        ("GET", "/api/netfile/downloads") => Reply::json(200, json!({"jobs": [job_done(), job_running()]})),
        ("POST", "/api/netfile/downloads") => Reply::json(201, json!({"jobs": [job_new()]})),
        ("DELETE", p) if p.ends_with("/file") => {
            Reply::json(200, json!({"deleted": {"name": "영상.mp4", "file_name": "영상.mp4", "bytes": 3400000}}))
        }
        ("DELETE", p) if p.starts_with("/api/netfile/downloads/") => Reply::json(200, json!({"result": "cancelling"})),
        _ => Reply::json(404, json!({"error": "not found"})),
    }
}

fn full_mock() -> Mock {
    Mock::start(full_route)
}

/// 토큰과 주소를 환경 변수로 넘긴다(이 PC의 설정은 테스트 실행기가 가린다).
fn env(url: &str) -> [(&str, &str); 2] {
    [("TYANGE_TOWER_URL", url), ("TYANGE_TOWER_TOKEN", TOKEN)]
}

/// 기록된 요청 중 `/api/health`를 빼고 모두 올바른 토큰을 보냈는지, 토큰이 경로·본문에 새지 않았는지 본다.
fn assert_authorized(mock: &Mock) {
    for req in mock.requests() {
        if req.path() == "/api/health" {
            continue;
        }
        assert_eq!(req.auth.as_deref(), Some(AUTH), "{} {}", req.method, req.target);
        assert!(!req.target.contains(TOKEN) && !req.body.contains(TOKEN));
    }
}

fn assert_no_secret(run: &Run) {
    assert!(!run.all_output().contains(TOKEN), "the token leaked into output:\n{}", run.all_output());
}

fn parse_json(run: &Run) -> Value {
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", run.stdout))
}

// ── 연결과 토큰 ───────────────────────────────────────────────

#[test]
fn status_checks_the_tower_and_the_token_and_never_prints_the_token() {
    let mock = full_mock();
    let run = tyange(&["tower", "status"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("tytower 연결됨"), "{}", run.stdout);
    assert!(run.stdout.contains("토큰: 맞음 (설정(TYANGE_TOWER_TOKEN))"), "{}", run.stdout);
    assert_no_secret(&run);
    assert_eq!(mock.count("GET", "/api/health"), 1);
    assert_eq!(mock.count("GET", "/api/whoami"), 1);
    assert_authorized(&mock);
}

#[test]
fn status_json_says_where_the_token_came_from_without_the_value() {
    let mock = full_mock();
    let run = tyange(&["--json", "tower", "status"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    let value = parse_json(&run);
    assert_eq!(value["reachable"], true);
    assert_eq!(value["token"], "ok");
    assert_eq!(value["token_source"], "config");
    assert_eq!(value["origin"], mock.url.as_str());
    assert_no_secret(&run);
}

#[test]
fn with_no_setting_the_token_comes_from_the_tower_token_file_on_this_pc() {
    let mock = full_mock();
    let config = scratch("tower-config");
    std::fs::write(config.join("tower-token"), format!("{TOKEN}\n")).unwrap();
    let config_str = config.display().to_string();
    let run = tyange(
        &["--json", "tower", "status"],
        &[("TYANGE_TOWER_URL", mock.url.as_str()), ("TYTOWER_CONFIG_DIR", config_str.as_str())],
    );
    assert!(run.ok(), "{}", run.stderr);
    let value = parse_json(&run);
    assert_eq!(value["token"], "ok");
    assert_eq!(value["token_source"], "file");
    assert_no_secret(&run);
    assert_authorized(&mock);
    std::fs::remove_dir_all(config).unwrap();
}

#[test]
fn a_remote_address_never_reads_the_token_file() {
    let config = scratch("remote-config");
    std::fs::write(config.join("tower-token"), TOKEN).unwrap();
    let config_str = config.display().to_string();
    // 원격 주소에서는 토큰 파일을 읽지 않으므로 토큰이 없다고 안내하고, 네트워크에 나가기 전에 멈춘다.
    let run = tyange(
        &["tower", "brain", "list"],
        &[("TYANGE_TOWER_URL", "http://tower.tailnet.example:47820"), ("TYTOWER_CONFIG_DIR", config_str.as_str())],
    );
    assert!(!run.ok());
    assert!(run.stderr.contains("TYANGE_TOWER_TOKEN"), "{}", run.stderr);
    assert!(run.stderr.contains("원격"), "{}", run.stderr);
    assert_no_secret(&run);
    std::fs::remove_dir_all(config).unwrap();
}

#[test]
fn a_tower_that_is_not_running_says_how_to_start_it() {
    let url = closed_port_url();
    for args in [vec!["tower", "status"], vec!["tower", "overview"], vec!["tower", "netfile", "jobs"]] {
        let run = tyange(&args, &[("TYANGE_TOWER_URL", url.as_str()), ("TYANGE_TOWER_TOKEN", TOKEN)]);
        assert_eq!(run.code, 1, "{args:?}");
        assert!(run.stderr.contains("실행 중이 아닙니다"), "{args:?}: {}", run.stderr);
        assert!(run.stderr.contains("TYTOWER_HEADLESS=1"), "{args:?}: {}", run.stderr);
        assert_no_secret(&run);
    }
}

#[test]
fn a_wrong_token_is_reported_and_the_value_is_not_echoed() {
    let mock = full_mock();
    let run = tyange(
        &["tower", "status"],
        &[("TYANGE_TOWER_URL", mock.url.as_str()), ("TYANGE_TOWER_TOKEN", "wrong-token-value-XYZ")],
    );
    assert_eq!(run.code, 1);
    assert!(run.stdout.contains("토큰: 맞지 않음"), "{}", run.stdout);
    assert!(run.stderr.contains("토큰이 맞지 않습니다"), "{}", run.stderr);
    assert!(!run.all_output().contains("wrong-token-value-XYZ"));
}

#[test]
fn with_no_token_at_all_status_says_it_is_missing() {
    let mock = full_mock();
    let empty = scratch("no-token");
    let empty_str = empty.display().to_string();
    let run = tyange(
        &["tower", "status"],
        &[("TYANGE_TOWER_URL", mock.url.as_str()), ("TYTOWER_CONFIG_DIR", empty_str.as_str())],
    );
    assert_eq!(run.code, 1);
    assert!(run.stdout.contains("토큰: 없음"), "{}", run.stdout);
    assert!(run.stderr.contains("TYANGE_TOWER_TOKEN"), "{}", run.stderr);
    assert_eq!(mock.count("GET", "/api/whoami"), 0, "no token, so no whoami call");
    std::fs::remove_dir_all(empty).unwrap();
}

// ── 개요 ──────────────────────────────────────────────────────

#[test]
fn overview_shows_each_source_and_one_source_on_its_own() {
    let mock = full_mock();
    let run = tyange(&["tower", "overview"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("GeekNews · 정상 · 1건"), "{}", run.stdout);
    assert!(run.stdout.contains("tyange.com · 실패 · 0건"), "{}", run.stdout);
    assert!(run.stdout.contains("Work · abc-us/abc-web-apps · 정상"), "{}", run.stdout);
    assert!(run.stdout.contains("관심사 소식 · 1건"), "{}", run.stdout);

    let one = tyange(&["tower", "overview", "--source", "hn"], &env(&mock.url));
    assert!(one.ok(), "{}", one.stderr);
    assert!(one.stdout.contains("Show HN: 무언가"), "{}", one.stdout);
    assert!(!one.stdout.contains("Rust 소식"), "only the chosen source is listed:\n{}", one.stdout);
    assert_authorized(&mock);
}

#[test]
fn overview_json_is_the_towers_answer_and_a_source_picks_its_part() {
    let mock = full_mock();
    let all = parse_json(&tyange(&["--json", "tower", "overview"], &env(&mock.url)));
    assert_eq!(all, overview());

    let hn = tyange(&["--json", "tower", "overview", "--source", "hn"], &env(&mock.url));
    assert!(hn.ok(), "{}", hn.stderr);
    assert_eq!(parse_json(&hn)["id"], "hn");
    assert_eq!(parse_json(&hn)["items"][0]["title"], "Show HN: 무언가");
    assert_no_secret(&hn);
}

#[test]
fn overview_refresh_starts_then_waits_until_the_refresh_is_done() {
    let gets = AtomicUsize::new(0);
    let mock = Mock::start(move |req| {
        if req.auth.as_deref() != Some(AUTH) {
            return Reply::json(401, json!({"error": "unauthorized"}));
        }
        match (req.method.as_str(), req.path()) {
            ("POST", "/api/overview/refresh") => Reply::json(202, json!({"status": "started"})),
            ("GET", "/api/overview") => {
                let n = gets.fetch_add(1, Ordering::SeqCst);
                let mut value = overview();
                value["refreshing"] = json!(n < 2);
                Reply::json(200, value)
            }
            _ => Reply::json(404, json!({"error": "not found"})),
        }
    });
    let run = tyange(&["tower", "overview", "--refresh"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    let seen = mock.requests();
    let post = seen.iter().position(|r| r.method == "POST").expect("a refresh was started");
    let first_get = seen.iter().position(|r| r.method == "GET").expect("the overview was read");
    assert!(post < first_get, "the refresh starts before anything is read");
    assert!(mock.count("GET", "/api/overview") >= 3, "it polled until refreshing turned off");
    assert!(run.stdout.contains("GeekNews · 정상"), "{}", run.stdout);
    assert!(!run.stdout.contains("갱신 중"), "the printed overview is the finished one");
}

#[test]
fn overview_refresh_while_one_is_running_waits_instead_of_failing() {
    let gets = AtomicUsize::new(0);
    let mock = Mock::start(move |req| match (req.method.as_str(), req.path()) {
        ("POST", "/api/overview/refresh") => Reply::json(409, json!({"error": "already_running"})),
        ("GET", "/api/overview") => {
            gets.fetch_add(1, Ordering::SeqCst);
            Reply::json(200, overview())
        }
        _ => Reply::json(404, json!({"error": "not found"})),
    });
    let mock_url = mock.url.clone();
    let run = tyange(&["tower", "overview", "--refresh"], &[("TYANGE_TOWER_URL", mock_url.as_str()), ("TYANGE_TOWER_TOKEN", TOKEN)]);
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stderr.contains("이미 갱신 중"), "{}", run.stderr);
    assert_eq!(mock.count("POST", "/api/overview/refresh"), 1);
}

// ── 브레인 ────────────────────────────────────────────────────

#[test]
fn brain_list_and_show_by_title_print_the_note_and_its_links() {
    let mock = full_mock();
    let list = tyange(&["tower", "brain", "list"], &env(&mock.url));
    assert!(list.ok(), "{}", list.stderr);
    assert!(list.stdout.contains("노트 2개 · 브레인: brain"), "{}", list.stdout);
    assert!(list.stdout.contains("03 자료/글.md"), "{}", list.stdout);

    let show = tyange(&["tower", "brain", "show", "글"], &env(&mock.url));
    assert!(show.ok(), "{}", show.stderr);
    assert!(show.stdout.contains("# 글"), "{}", show.stdout);
    assert!(show.stdout.contains("본문 rust 이야기"), "{}", show.stdout);
    assert!(show.stdout.contains("[파일] pic.png → tower brain file ZmlsZS1pZA -o"), "{}", show.stdout);
    assert!(show.stdout.contains("[노트] 두 번째 → id c2Vjb25k"), "{}", show.stdout);
    assert!(show.stdout.contains("이 노트를 가리키는 노트"), "{}", show.stdout);
    assert_eq!(mock.count("GET", &format!("/api/brain/notes/{NOTE_A}")), 1, "the note was fetched by its id");
    assert_authorized(&mock);
}

#[test]
fn brain_json_passes_the_answers_through() {
    let mock = full_mock();
    assert_eq!(parse_json(&tyange(&["--json", "tower", "brain", "list"], &env(&mock.url))), index());
    assert_eq!(parse_json(&tyange(&["--json", "tower", "brain", "show", "두 번째"], &env(&mock.url))), note_b());
    assert_eq!(parse_json(&tyange(&["--json", "tower", "brain", "show", NOTE_A], &env(&mock.url))), note_a());
}

#[test]
fn brain_show_refuses_an_ambiguous_title_and_lists_the_candidates() {
    let mock = Mock::start(|req| match req.path() {
        "/api/brain/notes" => Reply::json(
            200,
            json!({"available": true, "root_name": "brain", "count": 2, "notes": [
                {"id": "id-a", "title": "메모", "folder": "a", "path": "a/메모.md", "modified": null},
                {"id": "id-b", "title": "메모", "folder": "b", "path": "b/메모.md", "modified": null}
            ]}),
        ),
        _ => Reply::json(404, json!({"error": "not_found"})),
    });
    let url = mock.url.clone();
    let run = tyange(&["tower", "brain", "show", "메모"], &[("TYANGE_TOWER_URL", url.as_str()), ("TYANGE_TOWER_TOKEN", TOKEN)]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("id-a") && run.stderr.contains("id-b"), "{}", run.stderr);
    assert_eq!(mock.count("GET", "/api/brain/notes/id-a"), 0, "nothing is fetched when the choice is unclear");
}

#[test]
fn brain_search_sends_the_query_and_the_limit() {
    let mock = full_mock();
    let run = tyange(&["tower", "brain", "search", "rust", "--limit", "5"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("\"rust\" 검색 결과 1건"), "{}", run.stdout);
    assert!(run.stdout.contains("본문 rust 이야기"), "{}", run.stdout);
    let search_req = mock.requests().into_iter().find(|r| r.path() == "/api/brain/search").unwrap();
    assert_eq!(search_req.query(), "q=rust&limit=5");
    assert_authorized(&mock);
}

#[test]
fn brain_file_writes_the_image_once_and_never_overwrites_it() {
    let mock = full_mock();
    let dir = scratch("brain-file");
    let out = dir.join("pic.png").display().to_string();
    let first = tyange(&["tower", "brain", "file", "ZmlsZS1pZA", "-o", out.as_str()], &env(&mock.url));
    assert!(first.ok(), "{}", first.stderr);
    assert_eq!(std::fs::read(&out).unwrap(), b"PNGDATA");

    let second = tyange(&["tower", "brain", "file", "ZmlsZS1pZA", "-o", out.as_str()], &env(&mock.url));
    assert_eq!(second.code, 1);
    assert!(second.stderr.contains("이미 있는 파일"), "{}", second.stderr);
    assert_eq!(std::fs::read(&out).unwrap(), b"PNGDATA", "the old file is untouched");

    let json_out = dir.join("again.png").display().to_string();
    let json = tyange(&["--json", "tower", "brain", "file", "ZmlsZS1pZA", "-o", json_out.as_str()], &env(&mock.url));
    assert!(json.ok(), "{}", json.stderr);
    assert_eq!(parse_json(&json)["bytes"], 7);
    assert_eq!(parse_json(&json)["content_type"], "image/png");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn brain_sync_starts_then_waits_for_the_result() {
    let polls = AtomicUsize::new(0);
    let mock = Mock::start(move |req| match (req.method.as_str(), req.path()) {
        ("POST", "/api/brain/sync") => Reply::json(202, sync_running()),
        ("GET", "/api/brain/sync") => {
            if polls.fetch_add(1, Ordering::SeqCst) < 1 {
                Reply::json(200, sync_running())
            } else {
                Reply::json(200, sync_done())
            }
        }
        _ => Reply::json(404, json!({"error": "not found"})),
    });
    let run = tyange(&["tower", "brain", "sync"], &[("TYANGE_TOWER_URL", mock.url.as_str()), ("TYANGE_TOWER_TOKEN", TOKEN)]);
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("동기화 끝 · 변경 기록 · 서버로 올림"), "{}", run.stdout);
    assert_eq!(mock.count("POST", "/api/brain/sync"), 1);
    assert!(mock.count("GET", "/api/brain/sync") >= 2);
}

#[test]
fn a_failed_brain_sync_is_an_error_with_the_reason() {
    let mock = Mock::start(|req| match (req.method.as_str(), req.path()) {
        ("POST", "/api/brain/sync") => Reply::json(202, sync_running()),
        ("GET", "/api/brain/sync") => Reply::json(
            200,
            json!({"status": "failed", "remote": "x", "auto_sync": false, "report": null, "error": "ssh: 연결 거부"}),
        ),
        _ => Reply::json(404, json!({"error": "not found"})),
    });
    let run = tyange(&["tower", "brain", "sync"], &[("TYANGE_TOWER_URL", mock.url.as_str()), ("TYANGE_TOWER_TOKEN", TOKEN)]);
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("브레인 동기화에 실패했습니다: ssh: 연결 거부"), "{}", run.stderr);
}

// ── NetFile ───────────────────────────────────────────────────

#[test]
fn netfile_status_says_whether_a_login_exists_and_nothing_more() {
    let mock = full_mock();
    let run = tyange(&["tower", "netfile", "status"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("있음 (환경 변수) — 값은 보이지 않습니다"), "{}", run.stdout);
    assert!(run.stdout.contains("받는 중 1 · 대기 0 (동시에 최대 2개)"), "{}", run.stdout);
    assert_no_secret(&run);
    assert_authorized(&mock);
}

#[test]
fn netfile_search_sends_the_same_filters_nf_search_takes() {
    let mock = full_mock();
    let run = tyange(
        &["tower", "netfile", "search", "회의", "--in", "예산", "--type", "doc", "--scope", "file", "--period", "1y"],
        &env(&mock.url),
    );
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stdout.contains("회의록.docx"), "{}", run.stdout);
    let sent = mock.requests().into_iter().find(|r| r.path() == "/api/netfile/search").unwrap().json();
    assert_eq!(
        sent,
        json!({"keyword": "회의", "filters": {"scope": "file", "kind": "doc", "period": "1y",
                                              "refine": {"mode": "include", "word": "예산"}}})
    );
    assert_no_secret(&run);
}

#[test]
fn netfile_ls_download_jobs_and_cancel_use_their_routes() {
    let mock = full_mock();
    let ls = tyange(&["tower", "netfile", "ls", "folder-1"], &env(&mock.url));
    assert!(ls.ok(), "{}", ls.stderr);
    assert!(ls.stdout.contains("회의록.docx"), "{}", ls.stdout);
    let ls_req = mock.requests().into_iter().find(|r| r.path() == "/api/netfile/ls").unwrap();
    assert_eq!(ls_req.json(), json!({"folder_id": "folder-1"}));

    let download = tyange(&["tower", "netfile", "download", "e1", "e2"], &env(&mock.url));
    assert!(download.ok(), "{}", download.stderr);
    assert!(download.stdout.contains("job job-new"), "{}", download.stdout);
    let started = mock.requests().into_iter().find(|r| r.method == "POST" && r.path() == "/api/netfile/downloads").unwrap();
    assert_eq!(started.json(), json!({"entry_ids": ["e1", "e2"]}));

    let jobs = tyange(&["tower", "netfile", "jobs"], &env(&mock.url));
    assert!(jobs.ok(), "{}", jobs.stderr);
    assert!(jobs.stdout.contains("job-run") && jobs.stdout.contains("5.0 MB / 20.0 MB (25%)"), "{}", jobs.stdout);
    assert!(jobs.stdout.contains("완료 · 3.4 MB"), "{}", jobs.stdout);

    let cancel = tyange(&["tower", "netfile", "cancel", "job-run"], &env(&mock.url));
    assert!(cancel.ok(), "{}", cancel.stderr);
    assert!(cancel.stdout.contains("취소를 걸었습니다"), "{}", cancel.stdout);
    assert_eq!(mock.count("DELETE", "/api/netfile/downloads/job-run"), 1);
    assert_authorized(&mock);
}

#[test]
fn rm_refuses_without_yes_and_names_the_file_and_its_size_first() {
    let mock = full_mock();
    let run = tyange(&["tower", "netfile", "rm", "job-done"], &env(&mock.url));
    assert_eq!(run.code, 1);
    assert!(run.stderr.contains("영상.mp4 (3.4 MB)"), "{}", run.stderr);
    assert!(run.stderr.contains("--yes"), "{}", run.stderr);
    assert!(mock.requests().iter().all(|r| r.method != "DELETE"), "nothing is deleted without --yes");
    assert_eq!(mock.count("GET", "/api/netfile/downloads"), 1, "the job list is read to name the file");
}

#[test]
fn rm_with_yes_deletes_the_completed_file_and_reports_it() {
    let mock = full_mock();
    let run = tyange(&["tower", "netfile", "rm", "job-done", "--yes"], &env(&mock.url));
    assert!(run.ok(), "{}", run.stderr);
    assert!(run.stderr.contains("지웁니다: 영상.mp4 (3.4 MB)"), "{}", run.stderr);
    assert!(run.stdout.contains("삭제했습니다: 영상.mp4 (3.4 MB)"), "{}", run.stdout);
    assert_eq!(mock.count("DELETE", "/api/netfile/downloads/job-done/file"), 1);
    assert_authorized(&mock);
}

#[test]
fn rm_never_deletes_a_file_that_is_still_downloading_or_an_unknown_job() {
    let mock = full_mock();
    let running = tyange(&["tower", "netfile", "rm", "job-run", "--yes"], &env(&mock.url));
    assert_eq!(running.code, 1);
    assert!(running.stderr.contains("다 받은 파일만"), "{}", running.stderr);

    let unknown = tyange(&["tower", "netfile", "rm", "no-such-job", "--yes"], &env(&mock.url));
    assert_eq!(unknown.code, 1);
    assert!(unknown.stderr.contains("작업을 찾지 못했습니다"), "{}", unknown.stderr);
    assert!(mock.requests().iter().all(|r| r.method != "DELETE"));
}

// ── 모든 새 명령: --json과 토큰 비노출 ───────────────────────────

#[test]
fn every_new_command_prints_json_with_the_flag_and_never_the_token() {
    let mock = full_mock();
    let dir = scratch("json-matrix");
    let image = dir.join("matrix.png").display().to_string();
    let cases: Vec<Vec<&str>> = vec![
        vec!["tower", "status"],
        vec!["tower", "overview"],
        vec!["tower", "overview", "--refresh"],
        vec!["tower", "overview", "--source", "geeknews"],
        vec!["tower", "overview", "--source", "work"],
        vec!["tower", "brain", "list"],
        vec!["tower", "brain", "show", "글"],
        vec!["tower", "brain", "search", "rust"],
        vec!["tower", "brain", "file", "ZmlsZS1pZA", "-o", image.as_str()],
        vec!["tower", "brain", "sync"],
        vec!["tower", "netfile", "status"],
        vec!["tower", "netfile", "search", "회의"],
        vec!["tower", "netfile", "ls", "f1"],
        vec!["tower", "netfile", "download", "e1"],
        vec!["tower", "netfile", "jobs"],
        vec!["tower", "netfile", "cancel", "job-run"],
        vec!["tower", "netfile", "rm", "job-done", "--yes"],
    ];
    for case in cases {
        let mut args = vec!["--json"];
        args.extend(case.iter().copied());
        let run = tyange(&args, &env(&mock.url));
        assert!(run.ok(), "{case:?}: {}", run.stderr);
        let _ = parse_json(&run);
        assert_no_secret(&run);
    }
    assert_authorized(&mock);
    std::fs::remove_dir_all(dir).unwrap();
}
