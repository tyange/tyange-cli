use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::AppError;
use crate::forgejo::{actionable, ActionRun, Forgejo, RunStatus};
use crate::git;
use crate::timeutil::{format_duration, now_secs, parse_rfc3339};

/// `ci --all` 한 줄. `--json`으로도 이 모양 그대로 나간다.
#[derive(Debug, serde::Serialize, PartialEq)]
pub struct Row {
    pub repo: String,
    pub status: RunStatus,
    pub branch: String,
    pub title: String,
    /// 시작(없으면 생성) 시각의 유닉스 초. 정렬과 경과 시간 계산용.
    pub since_secs: Option<i64>,
    /// 끝났으면 마지막 갱신 시각의 유닉스 초.
    pub updated_secs: Option<i64>,
    pub url: String,
}

/// 저장소 하나의 최근 실행에서 보여 줄 줄을 고른다. 기본은 돌고 있는 것(대기·실행 중·승인 대기)만,
/// `recent`면 상태와 관계없이 가장 최근 하나.
pub fn select_rows(repo: &str, runs: &[ActionRun], recent: bool) -> Vec<Row> {
    let picked: Vec<&ActionRun> = if recent {
        runs.iter().take(1).collect()
    } else {
        runs.iter().filter(|run| run.status.is_in_progress()).collect()
    };
    picked
        .into_iter()
        .map(|run| Row {
            repo: repo.to_string(),
            status: run.status,
            branch: run.prettyref.clone(),
            title: run.title.clone(),
            since_secs: parse_rfc3339(&run.started).or_else(|| parse_rfc3339(&run.created)),
            updated_secs: parse_rfc3339(&run.updated),
            url: run.html_url.clone(),
        })
        .collect()
}

fn clip(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// 돌고 있으면 "12분째", 끝났으면 "5분 전".
pub fn when_label(row: &Row, now: i64) -> String {
    if row.status.is_in_progress() {
        match row.since_secs {
            Some(since) => format!("{}째", format_duration(now - since)),
            None => "-".to_string(),
        }
    } else {
        match row.updated_secs {
            Some(updated) => format!("{} 전", format_duration(now - updated)),
            None => "-".to_string(),
        }
    }
}

pub fn format_row(row: &Row, now: i64, repo_width: usize) -> String {
    format!(
        "{:<repo_width$}  {:<9}  {:<10}  {:>10}  {}",
        row.repo,
        row.status.label(),
        clip(&row.branch, 10),
        when_label(row, now),
        clip(&row.title, 46),
    )
}

/// `tyange ci --all` — 접근할 수 있는 모든 저장소에서 지금 돌고 있는 CI를 모아 본다(`recent`면 저장소마다 마지막 실행).
pub fn run_all(recent: bool, json: bool, config: &Config) -> Result<(), AppError> {
    let host = config.forgejo_host();
    let forgejo = Forgejo::new(host, config.forgejo_token()?)?;

    let repos = actionable(forgejo.list_repos()?);
    let mut rows = Vec::new();
    let mut failed = Vec::new();
    for (repo, result) in forgejo.recent_runs_of(&repos, 10) {
        match result {
            Ok(runs) => rows.extend(select_rows(&repo, &runs, recent)),
            Err(error) => failed.push(format!("{repo}: {error}")),
        }
    }
    // 오래 돈 것이 위로. 시작 시각을 모르면 맨 아래.
    rows.sort_by_key(|row| (row.since_secs.is_none(), row.since_secs));
    if recent {
        rows.sort_by_key(|row| std::cmp::Reverse(row.updated_secs));
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!("{}", if recent { "실행 기록이 없습니다." } else { "지금 돌고 있는 CI가 없습니다." });
    } else {
        let now = now_secs();
        let width = rows.iter().map(|row| row.repo.chars().count()).max().unwrap_or(0);
        for row in &rows {
            println!("{}", format_row(row, now, width));
        }
    }
    for line in &failed {
        eprintln!("조회 실패 — {line}");
    }
    if !json {
        eprintln!("저장소 {}개 확인", repos.len());
    }
    Ok(())
}


const POLL: Duration = Duration::from_secs(5);
/// 푸시 직후엔 HEAD 커밋의 실행이 아직 목록에 없을 수 있다. 이만큼은 기다린다.
const APPEAR_WAIT: Duration = Duration::from_secs(180);

/// 지켜볼 실행을 고른다. 실행들이 커밋 해시를 알려 주면 HEAD의 것만, 아니면 가장 최근 것.
/// HEAD의 실행이 아직 없으면 `None`(옛 실행을 새 실행으로 착각하지 않게).
pub fn pick_run<'a>(runs: &'a [ActionRun], head: Option<&str>) -> Option<&'a ActionRun> {
    let knows_commits = runs.iter().any(|run| run.commit_sha.is_some());
    match (head, knows_commits) {
        (Some(head), true) => runs.iter().find(|run| {
            run.commit_sha
                .as_deref()
                .is_some_and(|sha| sha.starts_with(head) || head.starts_with(sha))
        }),
        _ => runs.first(),
    }
}

fn line(run: &ActionRun) -> String {
    format!(
        "{:<9} {} ({}) {}",
        run.status.label(),
        run.title,
        run.event,
        run.html_url
    )
}

fn outcome(run: &ActionRun) -> Result<(), AppError> {
    match run.status {
        RunStatus::Success | RunStatus::Skipped => Ok(()),
        status => Err(AppError::RunFailed(format!(
            "실행이 {}(으)로 끝났습니다: {}",
            status.label(),
            run.html_url
        ))),
    }
}

/// `tyange ci` — 현재 리포의 Forgejo Actions 최근 실행 상태를 본다.
pub fn run(watch: bool, timeout_mins: u64, json: bool, config: &Config) -> Result<(), AppError> {
    let url = git::origin_url()?;
    let info = git::parse_remote_url(&url).ok_or(AppError::UnrecognizedRemote(url))?;
    let forgejo = Forgejo::new(&info.host, config.forgejo_token()?)?;

    if !watch {
        let runs = forgejo.recent_runs(&info.owner, &info.repo, 5)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&runs)?);
        } else if runs.is_empty() {
            println!("실행 기록이 없습니다.");
        } else {
            for run in &runs {
                println!("{}", line(run));
            }
        }
        return Ok(());
    }

    let head = git::head_sha();
    let started = Instant::now();
    let deadline = Duration::from_secs(timeout_mins * 60);
    let mut last = String::new();
    loop {
        let runs = forgejo.recent_runs(&info.owner, &info.repo, 10)?;
        match pick_run(&runs, head.as_deref()) {
            Some(run) => {
                let now = line(run);
                if now != last {
                    println!("{now}");
                    last = now;
                }
                if run.status.is_finished() {
                    return outcome(run);
                }
            }
            None if started.elapsed() > APPEAR_WAIT => {
                return Err(AppError::RunFailed(
                    "HEAD 커밋의 실행이 3분이 지나도 나타나지 않았습니다. 푸시했는지, 워크플로가 이 브랜치에 걸려 있는지 확인하세요".into(),
                ));
            }
            None => {
                if last.is_empty() {
                    println!("HEAD 커밋의 실행을 기다리는 중...");
                    last = "waiting-for-run".into();
                }
            }
        }
        if started.elapsed() > deadline {
            return Err(AppError::RunFailed(format!(
                "{timeout_mins}분 안에 끝나지 않았습니다"
            )));
        }
        sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(status: RunStatus, sha: Option<&str>) -> ActionRun {
        ActionRun {
            status,
            title: "t".into(),
            event: "push".into(),
            updated: String::new(),
            html_url: "https://x/1".into(),
            commit_sha: sha.map(str::to_string),
            prettyref: String::new(),
            started: String::new(),
            created: String::new(),
            repository: None,
        }
    }

    #[test]
    fn picks_the_run_for_head_when_commits_are_known() {
        let runs = vec![run(RunStatus::Success, Some("aaaa111")), run(RunStatus::Running, Some("bbbb222"))];
        let picked = pick_run(&runs, Some("bbbb222ffff")).unwrap();
        assert_eq!(picked.status, RunStatus::Running);
    }

    #[test]
    fn does_not_mistake_an_old_run_for_the_new_one() {
        let runs = vec![run(RunStatus::Success, Some("aaaa111"))];
        assert!(pick_run(&runs, Some("cccc333")).is_none());
    }

    #[test]
    fn falls_back_to_the_latest_run_when_commits_are_unknown() {
        let runs = vec![run(RunStatus::Running, None), run(RunStatus::Success, None)];
        assert_eq!(pick_run(&runs, Some("abc")).unwrap().status, RunStatus::Running);
        assert!(pick_run(&[], Some("abc")).is_none());
    }

    fn timed(status: RunStatus, started: &str, updated: &str) -> ActionRun {
        ActionRun {
            started: started.into(),
            updated: updated.into(),
            prettyref: "main".into(),
            ..run(status, None)
        }
    }

    #[test]
    fn by_default_only_in_progress_runs_are_listed() {
        let runs = vec![
            timed(RunStatus::Success, "2026-10-15T05:00:00Z", "2026-10-15T05:03:00Z"),
            timed(RunStatus::Running, "2026-10-15T05:10:00Z", "2026-10-15T05:10:00Z"),
            timed(RunStatus::Waiting, "", ""),
            timed(RunStatus::Failure, "2026-10-14T05:00:00Z", "2026-10-14T05:03:00Z"),
        ];
        let rows = select_rows("o/r", &runs, false);
        assert_eq!(rows.iter().map(|r| r.status).collect::<Vec<_>>(), [RunStatus::Running, RunStatus::Waiting]);
        assert!(select_rows("o/r", &runs[..1], false).is_empty());
    }

    #[test]
    fn recent_takes_the_latest_run_whatever_its_state() {
        let runs = vec![
            timed(RunStatus::Failure, "2026-10-15T05:00:00Z", "2026-10-15T05:03:00Z"),
            timed(RunStatus::Success, "2026-10-14T05:00:00Z", "2026-10-14T05:03:00Z"),
        ];
        let rows = select_rows("o/r", &runs, true);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, RunStatus::Failure);
        assert_eq!(rows[0].repo, "o/r");
        assert!(select_rows("o/r", &[], true).is_empty());
    }

    #[test]
    fn rows_say_how_long_they_have_been_running_or_since_they_finished() {
        let now = parse_rfc3339("2026-10-15T05:22:00Z").unwrap();
        let running = &select_rows("o/r", &[timed(RunStatus::Running, "2026-10-15T05:10:00Z", "")], false)[0];
        assert_eq!(when_label(running, now), "12분째");
        let done = &select_rows("o/r", &[timed(RunStatus::Success, "2026-10-15T05:00:00Z", "2026-10-15T05:17:00Z")], true)[0];
        assert_eq!(when_label(done, now), "5분 전");
        // 시작 시각이 비어 있으면 생성 시각으로 대신하고, 둘 다 없으면 "-".
        let unknown = &select_rows("o/r", &[timed(RunStatus::Waiting, "", "")], false)[0];
        assert_eq!(when_label(unknown, now), "-");
    }

    #[test]
    fn a_formatted_row_lines_up_and_clips_long_titles() {
        let now = parse_rfc3339("2026-10-15T05:22:00Z").unwrap();
        let mut run = timed(RunStatus::Running, "2026-10-15T05:10:00Z", "");
        run.title = "가".repeat(60);
        let row = &select_rows("tyange-labs/lovelog", &[run], false)[0];
        let line = format_row(row, now, 24);
        assert!(line.starts_with("tyange-labs/lovelog       running    main"), "{line}");
        assert!(line.contains("12분째"), "{line}");
        assert!(line.ends_with('…'), "{line}");
    }

    #[test]
    fn only_success_and_skipped_count_as_ok() {
        assert!(outcome(&run(RunStatus::Success, None)).is_ok());
        assert!(outcome(&run(RunStatus::Skipped, None)).is_ok());
        assert!(outcome(&run(RunStatus::Failure, None)).is_err());
        assert!(outcome(&run(RunStatus::Cancelled, None)).is_err());
    }
}
