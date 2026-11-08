use std::io::Write;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::cli::BrainCommand;
use crate::commands::boards;
use crate::commands::tower::{human_bytes, print_json, wait_for, when, POLL, SYNC_WAIT};
use crate::error::AppError;
use crate::tower::Tower;

/// `GET /api/brain/notes`의 사람이 읽을 부분.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NotesIndex {
    pub available: bool,
    pub root_name: String,
    pub count: usize,
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub folder: String,
    pub path: String,
    pub modified: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NoteDetail {
    pub id: String,
    pub title: String,
    pub folder: String,
    pub path: String,
    pub modified: Option<String>,
    pub markdown: String,
    pub links: Vec<LinkRef>,
    pub backlinks: Vec<NoteRef>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LinkRef {
    pub href: String,
    pub kind: String,
    pub id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NoteRef {
    pub id: String,
    pub title: String,
    pub folder: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchResult {
    pub available: bool,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub folder: String,
    pub in_title: bool,
    pub snippet: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SyncStatus {
    pub status: String,
    pub report: Option<SyncReport>,
    pub error: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SyncReport {
    pub committed: bool,
    pub pulled: bool,
    pub pushed: bool,
    pub cloned: bool,
    pub conflict_copies: Vec<String>,
}

const NO_FOLDER_HINT: &str = "브레인 폴더가 이 PC에 없습니다. tytower의 TYTOWER_BRAIN_DIR이나 설정의 [brain] dir을 확인하세요";

pub fn run(tower: &Tower, command: BrainCommand, json: bool) -> Result<(), AppError> {
    match command {
        BrainCommand::List => {
            let value = tower.brain_index()?;
            if json {
                return print_json(&value);
            }
            let index: NotesIndex = serde_json::from_value(value)?;
            for line in list_lines(&index) {
                println!("{line}");
            }
        }
        BrainCommand::Show { note } => {
            let index: NotesIndex = serde_json::from_value(tower.brain_index()?)?;
            if !index.available {
                return Err(AppError::Invalid(NO_FOLDER_HINT.into()));
            }
            let summary = resolve_note(&index.notes, &note)?;
            let value = tower.brain_note(&summary.id)?;
            if json {
                return print_json(&value);
            }
            let detail: NoteDetail = serde_json::from_value(value)?;
            for line in detail_lines(&detail) {
                println!("{line}");
            }
        }
        BrainCommand::Search { query, limit } => {
            let value = tower.brain_search(&query, limit)?;
            if json {
                return print_json(&value);
            }
            let result: SearchResult = serde_json::from_value(value)?;
            for line in search_lines(&result, &query) {
                println!("{line}");
            }
        }
        BrainCommand::File { id, output } => {
            let (content_type, bytes) = tower.brain_file(&id)?;
            write_new_file(&output, &bytes)?;
            if json {
                return print_json(&serde_json::json!({
                    "path": output,
                    "bytes": bytes.len(),
                    "content_type": content_type,
                }));
            }
            println!("저장했습니다: {} ({})", output.display(), human_bytes(bytes.len() as u64));
        }
        BrainCommand::Sync => {
            if !tower.start_brain_sync()? {
                eprintln!("이미 동기화 중입니다. 끝날 때까지 기다립니다.");
            }
            let value = wait_for(POLL, SYNC_WAIT, "브레인 동기화", || tower.brain_sync_status(), sync_done)?;
            if json {
                return print_json(&value);
            }
            let status: SyncStatus = serde_json::from_value(value)?;
            if status.status == "failed" {
                return Err(AppError::RunFailed(format!(
                    "브레인 동기화에 실패했습니다: {}",
                    status.error.as_deref().unwrap_or("이유를 알 수 없음")
                )));
            }
            for line in sync_lines(&status) {
                println!("{line}");
            }
        }
        BrainCommand::Boards { command } => return boards::run(tower, command, json),
    }
    Ok(())
}

/// 동기화가 끝났는가(`running`이 아니면 끝난 것이다).
pub fn sync_done(status: &Value) -> bool {
    status["status"] != "running"
}

/// 노트를 고른다. id가 정확히 맞으면 그것, 아니면 제목·경로(확장자 없이도, 대소문자 무시)로 찾는다.
/// 여러 개가 맞으면 목록을 보여 주고 id로 고르게 한다.
pub fn resolve_note<'a>(notes: &'a [NoteSummary], query: &str) -> Result<&'a NoteSummary, AppError> {
    let q = query.trim();
    if let Some(exact) = notes.iter().find(|n| n.id == q) {
        return Ok(exact);
    }
    let wanted = q.to_lowercase();
    let matches: Vec<&NoteSummary> = notes
        .iter()
        .filter(|n| {
            let path = n.path.to_lowercase();
            let without_ext = path.strip_suffix(".md").unwrap_or(&path);
            n.title.trim().to_lowercase() == wanted || path == wanted || without_ext == wanted
        })
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => Err(AppError::Invalid(format!(
            "노트를 찾지 못했습니다: {q} (tower brain list로 확인하세요)"
        ))),
        many => {
            let list = many
                .iter()
                .map(|n| format!("  {}  {}", n.id, n.path))
                .collect::<Vec<_>>()
                .join("\n");
            Err(AppError::Invalid(format!("여러 노트가 맞습니다. id로 골라 주세요:\n{list}")))
        }
    }
}

/// 폴더 표시: 루트의 노트(폴더가 없음)는 아무것도 붙이지 않는다.
fn folder_suffix(folder: &str) -> String {
    if folder.trim().is_empty() {
        String::new()
    } else {
        format!("  ({folder})")
    }
}

pub fn list_lines(index: &NotesIndex) -> Vec<String> {
    if !index.available {
        return vec![NO_FOLDER_HINT.to_string()];
    }
    let mut lines = vec![format!("노트 {}개 · 브레인: {}", index.count, index.root_name)];
    for note in &index.notes {
        let modified = note.modified.as_deref().map(when).unwrap_or_else(|| "-".into());
        lines.push(format!("{modified:<11}  {}", note.path));
    }
    lines
}

pub fn detail_lines(note: &NoteDetail) -> Vec<String> {
    let mut lines = vec![
        format!("# {}", note.title),
        format!(
            "{} · 수정 {}",
            note.path,
            note.modified.as_deref().map(when).unwrap_or_else(|| "-".into())
        ),
        format!("id: {}", note.id),
        String::new(),
        note.markdown.clone(),
    ];
    if !note.links.is_empty() {
        lines.push(String::new());
        lines.push("링크:".into());
        for link in &note.links {
            lines.push(match (link.kind.as_str(), &link.id) {
                ("file", Some(id)) => format!("  [파일] {} → tower brain file {id} -o <경로>", link.href),
                ("note", Some(id)) => format!("  [노트] {} → id {id}", link.href),
                (kind, _) => format!("  [{kind}] {}", link.href),
            });
        }
    }
    if !note.backlinks.is_empty() {
        lines.push(String::new());
        lines.push("이 노트를 가리키는 노트:".into());
        for back in &note.backlinks {
            lines.push(format!("  {}{}", back.title, folder_suffix(&back.folder)));
        }
    }
    lines
}

pub fn search_lines(result: &SearchResult, query: &str) -> Vec<String> {
    if !result.available {
        return vec![NO_FOLDER_HINT.to_string()];
    }
    let mut lines = vec![format!("\"{query}\" 검색 결과 {}건", result.hits.len())];
    for hit in &result.hits {
        let title_mark = if hit.in_title { " [제목 일치]" } else { "" };
        lines.push(format!("  {}{title_mark}{}", hit.title, folder_suffix(&hit.folder)));
        if let Some(snippet) = &hit.snippet {
            lines.push(format!("      {}", snippet.split_whitespace().collect::<Vec<_>>().join(" ")));
        }
    }
    lines
}

pub fn sync_lines(status: &SyncStatus) -> Vec<String> {
    let Some(report) = &status.report else {
        return vec!["동기화 끝 · 바뀐 것 없음".to_string()];
    };
    let mut parts = Vec::new();
    if report.cloned {
        parts.push("처음 내려받음");
    }
    if report.committed {
        parts.push("변경 기록");
    }
    if report.pulled {
        parts.push("서버에서 받아 합침");
    }
    if report.pushed {
        parts.push("서버로 올림");
    }
    let mut lines = vec![format!(
        "동기화 끝 · {}",
        if parts.is_empty() { "바뀐 것 없음".to_string() } else { parts.join(" · ") }
    )];
    for copy in &report.conflict_copies {
        lines.push(format!("충돌 사본: {copy}"));
    }
    lines
}

/// 이미 있는 파일은 덮어쓰지 않는다(이미지를 실수로 지우지 않게).
fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                AppError::Invalid(format!("이미 있는 파일입니다. 다른 경로를 고르세요: {}", path.display()))
            }
            _ => AppError::Io(format!("{}: {e}", path.display())),
        })?;
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn note(id: &str, title: &str, path: &str) -> NoteSummary {
        NoteSummary {
            id: id.into(),
            title: title.into(),
            folder: path.rsplit_once('/').map(|(f, _)| f.to_string()).unwrap_or_default(),
            path: path.into(),
            modified: Some("2026-11-08T04:09:40Z".into()),
        }
    }

    #[test]
    fn a_note_is_found_by_id_title_or_path_without_the_extension() {
        let notes = vec![note("MDMvIOyekOujjC_uucu.md", "글", "03 자료/글.md"), note("b2", "두 번째", "두 번째.md")];
        assert_eq!(resolve_note(&notes, "b2").unwrap().title, "두 번째");
        assert_eq!(resolve_note(&notes, "글").unwrap().id, "MDMvIOyekOujjC_uucu.md");
        assert_eq!(resolve_note(&notes, "03 자료/글").unwrap().id, "MDMvIOyekOujjC_uucu.md");
        assert_eq!(resolve_note(&notes, "두 번째.MD").unwrap().title, "두 번째", "대소문자는 가리지 않는다");
        assert!(resolve_note(&notes, "없는 노트").is_err());
    }

    #[test]
    fn an_ambiguous_title_lists_the_candidates_instead_of_guessing() {
        let notes = vec![note("id-a", "메모", "a/메모.md"), note("id-b", "메모", "b/메모.md")];
        let error = resolve_note(&notes, "메모").unwrap_err().to_string();
        assert!(error.contains("id-a") && error.contains("id-b"), "{error}");
        assert_eq!(resolve_note(&notes, "b/메모").unwrap().id, "id-b", "a path picks one");
    }

    #[test]
    fn the_list_shows_dates_and_paths_or_a_hint_without_the_folder() {
        let index = NotesIndex {
            available: true,
            root_name: "brain".into(),
            count: 1,
            notes: vec![note("x", "글", "03 자료/글.md")],
        };
        let lines = list_lines(&index);
        assert_eq!(lines[0], "노트 1개 · 브레인: brain");
        assert!(lines[1].ends_with("03 자료/글.md"), "{lines:?}");
        let missing = NotesIndex { available: false, ..Default::default() };
        assert!(list_lines(&missing)[0].contains("TYTOWER_BRAIN_DIR"));
    }

    #[test]
    fn a_note_shows_its_body_links_and_backlinks() {
        let detail: NoteDetail = serde_json::from_value(json!({
            "id": "i1", "title": "글", "folder": "03 자료", "path": "03 자료/글.md", "modified": null,
            "markdown": "# 제목\n본문",
            "links": [
                {"href": "두 번째", "kind": "note", "id": "i2"},
                {"href": "pic.png", "kind": "file", "id": "f1"},
                {"href": "https://x", "kind": "external", "id": null},
                {"href": "#없음", "kind": "unresolved", "id": null}
            ],
            "backlinks": [{"id": "i2", "title": "두 번째", "folder": ""}]
        }))
        .unwrap();
        let text = detail_lines(&detail).join("\n");
        assert!(text.starts_with("# 글\n"), "{text}");
        assert!(text.contains("# 제목\n본문"));
        assert!(text.contains("[파일] pic.png → tower brain file f1 -o <경로>"), "{text}");
        assert!(text.contains("[노트] 두 번째 → id i2"));
        assert!(text.contains("[external] https://x") && text.contains("[unresolved] #없음"));
        assert!(text.ends_with("  두 번째"), "a root-level note has no empty folder marker: {text}");
    }

    #[test]
    fn search_marks_title_hits_and_flattens_snippets() {
        let result: SearchResult = serde_json::from_value(json!({
            "q": "rust", "available": true,
            "hits": [
                {"id": "a", "title": "Rust 노트", "folder": "dev", "in_title": true, "snippet": null},
                {"id": "b", "title": "메모", "folder": "", "in_title": false, "snippet": "앞\n\n rust   이야기"}
            ]
        }))
        .unwrap();
        let lines = search_lines(&result, "rust");
        assert_eq!(lines[0], "\"rust\" 검색 결과 2건");
        assert_eq!(lines[1], "  Rust 노트 [제목 일치]  (dev)");
        assert_eq!(lines[3], "      앞 rust 이야기");
        assert_eq!(lines[2], "  메모", "a root-level hit has no empty folder marker");
    }

    #[test]
    fn sync_reports_what_happened_and_the_conflict_copies() {
        let status: SyncStatus = serde_json::from_value(json!({
            "status": "done",
            "report": {"committed": true, "pulled": true, "pushed": true, "cloned": false,
                       "conflict_copies": ["글 (충돌 사본 2026-11-08).md"]},
            "error": null
        }))
        .unwrap();
        let lines = sync_lines(&status);
        assert_eq!(lines[0], "동기화 끝 · 변경 기록 · 서버에서 받아 합침 · 서버로 올림");
        assert_eq!(lines[1], "충돌 사본: 글 (충돌 사본 2026-11-08).md");
        let quiet: SyncStatus = serde_json::from_value(json!({"status": "done", "report": null})).unwrap();
        assert_eq!(sync_lines(&quiet), vec!["동기화 끝 · 바뀐 것 없음".to_string()]);
    }

    #[test]
    fn a_sync_is_done_once_it_is_no_longer_running() {
        assert!(!sync_done(&json!({"status": "running"})));
        assert!(sync_done(&json!({"status": "done"})));
        assert!(sync_done(&json!({"status": "failed"})));
    }

    #[test]
    fn a_file_is_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("tyange-brain-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pic.png");
        let _ = std::fs::remove_file(&path);
        write_new_file(&path, b"first").unwrap();
        let again = write_new_file(&path, b"second").unwrap_err().to_string();
        assert!(again.contains("이미 있는 파일"), "{again}");
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
