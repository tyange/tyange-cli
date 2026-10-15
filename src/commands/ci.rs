use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::error::AppError;
use crate::forgejo::{ActionRun, Forgejo, RunStatus};
use crate::git;

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

    #[test]
    fn only_success_and_skipped_count_as_ok() {
        assert!(outcome(&run(RunStatus::Success, None)).is_ok());
        assert!(outcome(&run(RunStatus::Skipped, None)).is_ok());
        assert!(outcome(&run(RunStatus::Failure, None)).is_err());
        assert!(outcome(&run(RunStatus::Cancelled, None)).is_err());
    }
}
