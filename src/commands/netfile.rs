use serde::Deserialize;
use serde_json::{json, Value};

use crate::cli::NetfileCommand;
use crate::commands::tower::{human_bytes, print_json};
use crate::error::AppError;
use crate::tower::Tower;

/// `GET /api/netfile/status`. 로그인 값은 "있다/없다"와 출처만 온다(값은 없다).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct NetfileStatus {
    pub credentials: Credentials,
    pub download_dir: String,
    pub running: u64,
    pub queued: u64,
    pub max_concurrent: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Credentials {
    pub state: String,
    pub source: Option<String>,
}

/// 검색·목록 응답의 한 줄 항목.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub size: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Listing {
    pub total: u64,
    pub more: bool,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Job {
    pub id: String,
    pub entry_id: String,
    pub name: String,
    pub file_name: String,
    pub state: String,
    pub bytes_done: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Jobs {
    pub jobs: Vec<Job>,
}

pub fn run(tower: &Tower, command: NetfileCommand, json: bool) -> Result<(), AppError> {
    match command {
        NetfileCommand::Status => {
            let value = tower.netfile_status()?;
            if json {
                return print_json(&value);
            }
            let status: NetfileStatus = serde_json::from_value(value)?;
            for line in status_lines(&status) {
                println!("{line}");
            }
        }
        NetfileCommand::Search { keyword, include, exclude, kind, period, size, scope } => {
            let body = search_body(
                &keyword,
                &scope,
                kind.as_deref(),
                period.as_deref(),
                size.as_deref(),
                include.as_deref(),
                exclude.as_deref(),
            )?;
            let value = tower.netfile_search(&body)?;
            if json {
                return print_json(&value);
            }
            let listing: Listing = serde_json::from_value(value)?;
            for line in listing_lines(&listing, &format!("\"{keyword}\" 검색")) {
                println!("{line}");
            }
        }
        NetfileCommand::Ls { folder_id } => {
            let value = tower.netfile_ls(&folder_id)?;
            if json {
                return print_json(&value);
            }
            let listing: Listing = serde_json::from_value(value)?;
            for line in listing_lines(&listing, &format!("폴더 {folder_id}")) {
                println!("{line}");
            }
        }
        NetfileCommand::Download { entry_ids } => {
            let value = tower.netfile_start(&entry_ids)?;
            if json {
                return print_json(&value);
            }
            let jobs: Jobs = serde_json::from_value(value)?;
            for line in download_lines(&jobs.jobs) {
                println!("{line}");
            }
        }
        NetfileCommand::Jobs => {
            let value = tower.netfile_jobs()?;
            if json {
                return print_json(&value);
            }
            let jobs: Jobs = serde_json::from_value(value)?;
            for line in jobs_lines(&jobs.jobs) {
                println!("{line}");
            }
        }
        NetfileCommand::Cancel { job_id } => {
            let value = tower.netfile_cancel(&job_id)?;
            if json {
                return print_json(&value);
            }
            println!("{}", cancel_message(value["result"].as_str().unwrap_or("")));
        }
        NetfileCommand::Rm { job_id, yes } => rm(tower, &job_id, yes, json)?,
    }
    Ok(())
}

/// 완료된 작업의 파일을 지운다. 파일 이름과 크기를 먼저 보여 주고, `--yes`가 있어야 지운다.
/// 지우기 전에 목록에서 작업을 찾아 상태를 확인하므로, 잘못된 id나 아직 받는 중인 파일은 DELETE를 보내지 않는다.
fn rm(tower: &Tower, job_id: &str, yes: bool, json: bool) -> Result<(), AppError> {
    let jobs: Jobs = serde_json::from_value(tower.netfile_jobs()?)?;
    let job = jobs
        .jobs
        .iter()
        .find(|j| j.id == job_id)
        .ok_or_else(|| AppError::Invalid(format!("작업을 찾지 못했습니다: {job_id} (tower netfile jobs로 확인하세요)")))?;
    let described = describe_file(job);
    if let Some(reason) = rm_refusal(job, yes) {
        return Err(AppError::Invalid(reason));
    }
    if !json {
        eprintln!("지웁니다: {described}");
    }
    let value = tower.netfile_delete_file(job_id)?;
    if json {
        return print_json(&value);
    }
    let deleted = &value["deleted"];
    println!(
        "삭제했습니다: {} ({})",
        deleted["file_name"].as_str().unwrap_or(&job.file_name),
        human_bytes(deleted["bytes"].as_u64().unwrap_or(job.bytes_done))
    );
    Ok(())
}

/// 지우면 안 되는 이유. `None`이면 지워도 된다. 이유 문구에 파일 이름과 크기를 넣는다.
pub fn rm_refusal(job: &Job, yes: bool) -> Option<String> {
    if job.state != "done" {
        return Some(format!(
            "다 받은 파일만 지울 수 있습니다. {} 상태: {}",
            job.file_name,
            state_label(&job.state)
        ));
    }
    if !yes {
        return Some(format!(
            "'{}'을(를) 지웁니다. 되돌릴 수 없으니 정말이면 --yes를 붙이세요",
            describe_file(job)
        ));
    }
    None
}

fn describe_file(job: &Job) -> String {
    format!("{} ({})", job.file_name, human_bytes(job.bytes_done))
}

/// `search` 요청 본문. 지정하지 않은 필터는 빼서 보낸다(tytower NetFile 화면과 같은 모양).
pub fn search_body(
    keyword: &str,
    scope: &str,
    kind: Option<&str>,
    period: Option<&str>,
    size: Option<&str>,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<Value, AppError> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err(AppError::Invalid("검색어를 입력해 주세요".into()));
    }
    let mut filters = json!({ "scope": scope });
    if let Some(kind) = kind {
        filters["kind"] = json!(kind);
    }
    if let Some(period) = period {
        filters["period"] = json!(period);
    }
    if let Some(size) = size {
        filters["size"] = json!(size);
    }
    match (include, exclude) {
        (Some(_), Some(_)) => return Err(AppError::Invalid("--in과 --not은 함께 쓸 수 없습니다".into())),
        (Some(word), None) => filters["refine"] = json!({ "mode": "include", "word": word.trim() }),
        (None, Some(word)) => filters["refine"] = json!({ "mode": "exclude", "word": word.trim() }),
        (None, None) => {}
    }
    Ok(json!({ "keyword": keyword, "filters": filters }))
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "folder" => "폴더",
        "file" => "파일",
        _ => "?",
    }
}

fn state_label(state: &str) -> &'static str {
    match state {
        "queued" => "대기 중",
        "running" => "받는 중",
        "done" => "완료",
        "failed" => "실패",
        _ => "알 수 없음",
    }
}

fn credentials_label(credentials: &Credentials) -> String {
    match credentials.state.as_str() {
        "ok" => {
            let source = match credentials.source.as_deref() {
                Some("environment") => "환경 변수",
                Some("local_file") => "로컬 파일",
                Some("os_keyring") => "OS 자격 증명 저장소",
                _ => "알 수 없는 출처",
            };
            format!("있음 ({source}) — 값은 보이지 않습니다")
        }
        "unreadable" => "읽을 수 없음".into(),
        _ => "없음 — 데스크톱에서 netfile 도구로 로그인해 저장한 뒤 다시 확인하세요".into(),
    }
}

pub fn status_lines(status: &NetfileStatus) -> Vec<String> {
    vec![
        format!("NetFile 로그인 값: {}", credentials_label(&status.credentials)),
        format!("다운로드 폴더: {}", status.download_dir),
        format!(
            "받는 중 {} · 대기 {} (동시에 최대 {}개)",
            status.running, status.queued, status.max_concurrent
        ),
    ]
}

/// 검색·폴더 목록 표. 더 있으면 알려 준다(검색어를 좁히라는 뜻).
pub fn listing_lines(listing: &Listing, title: &str) -> Vec<String> {
    let mut lines = vec![format!("{title}: {}건{}", listing.total, if listing.more { " (더 있음 — 검색어를 좁히세요)" } else { "" })];
    if listing.entries.is_empty() {
        lines.push("  결과가 없습니다.".into());
        return lines;
    }
    lines.push(format!("  {:<4}  {:>10}  {:<22}  {}", "종류", "크기", "id", "이름"));
    for entry in &listing.entries {
        lines.push(format!(
            "  {:<4}  {:>10}  {:<22}  {}",
            kind_label(&entry.kind),
            if entry.size.trim().is_empty() { "-" } else { entry.size.trim() },
            entry.id,
            entry.name
        ));
    }
    lines
}

/// 진행 상황 한 줄: 완료면 크기, 받는 중이면 "받은 양 / 전체 (%)", 모르면 받은 양만.
pub fn progress_text(job: &Job) -> String {
    match job.state.as_str() {
        "done" => format!("완료 · {}", human_bytes(job.bytes_done)),
        "failed" => job.message.clone().unwrap_or_else(|| "실패".into()),
        _ => match job.total {
            Some(total) if total > 0 => {
                let pct = (job.bytes_done as f64 / total as f64 * 100.0).min(100.0);
                format!("{} / {} ({pct:.0}%)", human_bytes(job.bytes_done), human_bytes(total))
            }
            _ => human_bytes(job.bytes_done),
        },
    }
}

pub fn jobs_lines(jobs: &[Job]) -> Vec<String> {
    if jobs.is_empty() {
        return vec!["받기 작업이 없습니다.".into()];
    }
    let mut lines = vec![format!("  {:<22}  {:<7}  {:<34}  {}", "job id", "상태", "진행", "이름")];
    for job in jobs {
        lines.push(format!(
            "  {:<22}  {:<7}  {:<34}  {}",
            job.id,
            state_label(&job.state),
            progress_text(job),
            job.name
        ));
    }
    lines
}

pub fn download_lines(jobs: &[Job]) -> Vec<String> {
    let mut lines = vec![format!("받기 작업 {}건을 만들었습니다.", jobs.len())];
    for job in jobs {
        lines.push(format!("  job {}  {}  {}", job.id, state_label(&job.state), job.name));
    }
    if !jobs.is_empty() {
        lines.push("취소: tower netfile cancel <job id> · 완료 후 지우기: tower netfile rm <job id> --yes".into());
    }
    lines
}

pub fn cancel_message(result: &str) -> String {
    match result {
        "cancelled" => "대기 중이던 작업을 취소했습니다.".into(),
        "cancelling" => "받는 중인 작업에 취소를 걸었습니다. 다음 조각에서 멈춥니다.".into(),
        "cleared" => "끝난 작업을 목록에서 지웠습니다. 받은 파일은 그대로 있습니다.".into(),
        other => format!("결과: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(state: &str, bytes_done: u64, total: Option<u64>) -> Job {
        Job {
            id: "job-1".into(),
            entry_id: "e1".into(),
            name: "영상.mp4".into(),
            file_name: "영상.mp4".into(),
            state: state.into(),
            bytes_done,
            total,
            message: None,
        }
    }

    #[test]
    fn a_search_sends_only_the_filters_that_were_given() {
        let body = search_body("회의록", "all", None, None, None, None, None).unwrap();
        assert_eq!(body, json!({"keyword": "회의록", "filters": {"scope": "all"}}));

        let body = search_body(" rust ", "file", Some("video"), Some("1w"), Some("100m-1g"), Some("book"), None).unwrap();
        assert_eq!(body["keyword"], "rust");
        assert_eq!(
            body["filters"],
            json!({"scope": "file", "kind": "video", "period": "1w", "size": "100m-1g", "refine": {"mode": "include", "word": "book"}})
        );

        let body = search_body("a", "all", None, None, None, None, Some("draft")).unwrap();
        assert_eq!(body["filters"]["refine"], json!({"mode": "exclude", "word": "draft"}));
    }

    #[test]
    fn a_search_needs_a_keyword_and_one_refine_at_most() {
        assert!(search_body("   ", "all", None, None, None, None, None).is_err());
        assert!(search_body("x", "all", None, None, None, Some("a"), Some("b")).is_err());
    }

    #[test]
    fn a_listing_shows_kind_size_id_and_name_and_says_when_there_is_more() {
        let listing: Listing = serde_json::from_value(json!({
            "total": 120, "more": true,
            "entries": [
                {"id": "f1", "kind": "folder", "name": "회의", "size": ""},
                {"id": "x2", "kind": "file", "name": "영상.mp4", "size": "1.2 GB"}
            ]
        }))
        .unwrap();
        let lines = listing_lines(&listing, "\"회의\" 검색");
        assert_eq!(lines[0], "\"회의\" 검색: 120건 (더 있음 — 검색어를 좁히세요)");
        assert!(lines[2].contains("폴더") && lines[2].contains("f1") && lines[2].ends_with("회의"), "{lines:?}");
        assert!(lines[3].contains("1.2 GB") && lines[3].contains("영상.mp4"), "{lines:?}");
        let empty = listing_lines(&Listing::default(), "x");
        assert_eq!(empty[1], "  결과가 없습니다.");
    }

    #[test]
    fn progress_reads_naturally_for_each_state() {
        assert_eq!(progress_text(&job("done", 12_000_000, None)), "완료 · 12.0 MB");
        assert_eq!(progress_text(&job("running", 5_000_000, Some(20_000_000))), "5.0 MB / 20.0 MB (25%)");
        assert_eq!(progress_text(&job("queued", 0, None)), "0 B");
        let mut failed = job("failed", 0, None);
        failed.message = Some("연결이 끊겼어요".into());
        assert_eq!(progress_text(&failed), "연결이 끊겼어요");
    }

    #[test]
    fn the_jobs_table_lists_each_job_id_for_cancel_and_rm() {
        let lines = jobs_lines(&[job("running", 1, Some(4))]);
        assert!(lines[1].contains("job-1") && lines[1].contains("받는 중"), "{lines:?}");
        assert_eq!(jobs_lines(&[]), vec!["받기 작업이 없습니다.".to_string()]);
    }

    #[test]
    fn rm_refuses_unfinished_files_and_files_without_yes_and_names_the_file() {
        let done = job("done", 3_400_000, None);
        assert!(rm_refusal(&done, true).is_none(), "done + --yes is allowed");
        let asking = rm_refusal(&done, false).unwrap();
        assert!(asking.contains("영상.mp4 (3.4 MB)") && asking.contains("--yes"), "{asking}");
        let running = rm_refusal(&job("running", 1, Some(9)), true).unwrap();
        assert!(running.contains("받는 중") && running.contains("다 받은 파일만"), "{running}");
    }

    #[test]
    fn cancel_results_have_a_sentence_each() {
        assert!(cancel_message("cancelled").contains("취소"));
        assert!(cancel_message("cancelling").contains("다음 조각"));
        assert!(cancel_message("cleared").contains("받은 파일은 그대로"));
    }

    #[test]
    fn status_says_only_whether_credentials_exist() {
        let status: NetfileStatus = serde_json::from_value(json!({
            "credentials": {"state": "ok", "source": "os_keyring"},
            "download_dir": "C:\\Users\\me\\Downloads\\tytower-netfile",
            "running": 1, "queued": 0, "max_concurrent": 2
        }))
        .unwrap();
        let text = status_lines(&status).join("\n");
        assert!(text.contains("있음 (OS 자격 증명 저장소) — 값은 보이지 않습니다"), "{text}");
        assert!(text.contains("받는 중 1 · 대기 0 (동시에 최대 2개)"), "{text}");

        let missing: NetfileStatus = serde_json::from_value(json!({"credentials": {"state": "missing"}})).unwrap();
        assert!(status_lines(&missing)[0].contains("없음"));
    }
}
