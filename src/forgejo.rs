use std::time::Duration;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Forgejo Actions 실행 상태. 모르는 값이 와도 파싱이 깨지지 않게 `Unknown`으로 받는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Waiting,
    Running,
    Success,
    Failure,
    Cancelled,
    Skipped,
    Blocked,
    #[serde(other)]
    Unknown,
}

impl RunStatus {
    /// 더 기다려도 바뀌지 않는 상태인가.
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            RunStatus::Success | RunStatus::Failure | RunStatus::Cancelled | RunStatus::Skipped
        )
    }

    /// 지금 돌고 있거나 곧 돌 실행(대기·실행 중·승인 대기).
    pub fn is_in_progress(self) -> bool {
        matches!(
            self,
            RunStatus::Waiting | RunStatus::Running | RunStatus::Blocked
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            RunStatus::Waiting => "waiting",
            RunStatus::Running => "running",
            RunStatus::Success => "success",
            RunStatus::Failure => "failure",
            RunStatus::Cancelled => "cancelled",
            RunStatus::Skipped => "skipped",
            RunStatus::Blocked => "blocked",
            RunStatus::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunRepo {
    pub full_name: String,
}

/// 실제 응답(`/repos/{owner}/{repo}/actions/runs`)에서 쓰는 필드만 읽는다. `event`와 `trigger_event`가
/// 둘 다 오므로 별칭(alias)을 걸면 중복 필드 에러가 난다 — `event`만 읽는다.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionRun {
    pub status: RunStatus,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub commit_sha: Option<String>,
    /// 브랜치나 태그 이름.
    #[serde(default)]
    pub prettyref: String,
    #[serde(default)]
    pub started: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub repository: Option<RunRepo>,
}

#[derive(Debug, Deserialize)]
struct ListRunsResponse {
    #[serde(default)]
    workflow_runs: Vec<ActionRun>,
}

pub fn parse_runs(body: &str) -> Result<Vec<ActionRun>, AppError> {
    Ok(serde_json::from_str::<ListRunsResponse>(body)?.workflow_runs)
}

#[derive(Debug, Clone, Deserialize)]
pub struct RepoInfo {
    pub full_name: String,
    #[serde(default)]
    pub has_actions: bool,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<RepoInfo>,
}

/// CI를 돌릴 수 있는 저장소만(Actions가 켜져 있고 보관(archived)되지 않은 것).
pub fn actionable(repos: Vec<RepoInfo>) -> Vec<RepoInfo> {
    repos
        .into_iter()
        .filter(|repo| repo.has_actions && !repo.archived)
        .collect()
}

const REPO_PAGE: usize = 50;
const MAX_REPO_PAGES: usize = 20;
const WORKERS: usize = 6;

pub struct Forgejo {
    http: Client,
    base: String,
    token: String,
}

impl Forgejo {
    pub fn new(host: &str, token: &str) -> Result<Self, AppError> {
        Ok(Self {
            http: Client::builder().timeout(Duration::from_secs(20)).build()?,
            base: format!("https://{host}/api/v1"),
            token: token.to_string(),
        })
    }

    fn get(&self, path: &str) -> Result<String, AppError> {
        let response = self
            .http
            .get(format!("{}{path}", self.base))
            .header("Authorization", format!("token {}", self.token))
            .send()?;
        let status = response.status();
        let body = response.text()?;
        if !status.is_success() {
            return Err(AppError::Api {
                status: status.as_u16(),
                message: body.chars().take(200).collect(),
            });
        }
        Ok(body)
    }

    /// 최근 실행 `limit`개(최신이 앞).
    pub fn recent_runs(
        &self,
        owner: &str,
        repo: &str,
        limit: u32,
    ) -> Result<Vec<ActionRun>, AppError> {
        parse_runs(&self.get(&format!(
            "/repos/{owner}/{repo}/actions/runs?limit={limit}"
        ))?)
    }

    /// 토큰으로 볼 수 있는 모든 저장소(페이지를 넘겨 가며).
    pub fn list_repos(&self) -> Result<Vec<RepoInfo>, AppError> {
        let mut all = Vec::new();
        for page in 1..=MAX_REPO_PAGES {
            let body = self.get(&format!("/repos/search?limit={REPO_PAGE}&page={page}"))?;
            let batch = serde_json::from_str::<SearchResponse>(&body)?.data;
            let count = batch.len();
            all.extend(batch);
            if count < REPO_PAGE {
                break;
            }
        }
        Ok(all)
    }

    /// 저장소 여러 개의 최근 실행을 병렬로 가져온다. 결과 순서는 입력 순서와 같다.
    pub fn recent_runs_of(
        &self,
        repos: &[RepoInfo],
        limit: u32,
    ) -> Vec<(String, Result<Vec<ActionRun>, AppError>)> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Mutex;

        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<Option<Result<Vec<ActionRun>, AppError>>>> =
            Mutex::new((0..repos.len()).map(|_| None).collect());

        std::thread::scope(|scope| {
            for _ in 0..WORKERS.min(repos.len()) {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(repo) = repos.get(index) else { break };
                    let result = self.get(&format!(
                        "/repos/{}/actions/runs?limit={limit}",
                        repo.full_name
                    ))
                    .and_then(|body| parse_runs(&body));
                    results.lock().expect("results lock")[index] = Some(result);
                });
            }
        });

        repos
            .iter()
            .zip(results.into_inner().expect("results lock"))
            .map(|(repo, result)| {
                (
                    repo.full_name.clone(),
                    result.unwrap_or_else(|| Err(AppError::Network("조회하지 못했습니다".into()))),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-15에 git.tyange.com에서 받은 실제 응답을 줄인 것. `event`와 `trigger_event`가 같이 온다.
    const REAL_RUN: &str = r#"{"total_count":1,"workflow_runs":[{
        "id":166,"title":"fix: 리스트의 손글씨 이미지를 글씨가 읽히는 크기로",
        "repository":{"id":4,"full_name":"tyange-labs/lovelog","name":"lovelog","owner":{"login":"tyange-labs"},"has_actions":true},
        "workflow_id":"deploy.yml","prettyref":"main",
        "commit_sha":"1d71c6544f3414ff70f35ba021a4e4f5f00f6e9e","event":"push","trigger_event":"push",
        "status":"success","started":"2026-10-15T05:28:29Z","stopped":"2026-10-15T05:31:38Z",
        "created":"2026-10-15T05:28:27Z","updated":"2026-10-15T05:31:38Z","duration":189000000000,
        "html_url":"https://git.tyange.com/tyange-labs/lovelog/actions/runs/18"}]}"#;

    #[test]
    fn parses_the_real_response_including_both_event_fields() {
        let runs = parse_runs(REAL_RUN).unwrap();
        let run = &runs[0];
        assert_eq!(run.status, RunStatus::Success);
        assert_eq!(run.event, "push");
        assert_eq!(run.prettyref, "main");
        assert_eq!(run.repository.as_ref().unwrap().full_name, "tyange-labs/lovelog");
        assert_eq!(run.started, "2026-10-15T05:28:29Z");
        assert!(run.commit_sha.as_deref().unwrap().starts_with("1d71c65"));
    }

    #[test]
    fn parses_a_run_list_and_tolerates_unknown_statuses() {
        let runs = parse_runs(
            r#"{"total_count":2,"workflow_runs":[
                {"status":"running","title":"feat: x","event":"push","updated":"2026-10-15T04:00:00Z","html_url":"https://git/x","commit_sha":"abc"},
                {"status":"something-new","title":"old","event":"pull_request"}
            ]}"#,
        )
        .unwrap();

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].status, RunStatus::Running);
        assert_eq!(runs[0].commit_sha.as_deref(), Some("abc"));
        assert_eq!(runs[1].status, RunStatus::Unknown);
        assert_eq!(runs[1].event, "pull_request");
    }

    #[test]
    fn in_progress_means_waiting_running_or_blocked_only() {
        for status in [RunStatus::Waiting, RunStatus::Running, RunStatus::Blocked] {
            assert!(status.is_in_progress(), "{status:?}");
        }
        for status in [RunStatus::Success, RunStatus::Failure, RunStatus::Cancelled, RunStatus::Skipped, RunStatus::Unknown] {
            assert!(!status.is_in_progress(), "{status:?}");
        }
    }

    #[test]
    fn only_actionable_repos_are_kept() {
        let body = r#"{"ok":true,"data":[
            {"full_name":"o/a","has_actions":true,"archived":false},
            {"full_name":"o/b","has_actions":false,"archived":false},
            {"full_name":"o/c","has_actions":true,"archived":true},
            {"full_name":"o/d"}]}"#;
        let repos = actionable(serde_json::from_str::<SearchResponse>(body).unwrap().data);
        assert_eq!(repos.iter().map(|r| r.full_name.as_str()).collect::<Vec<_>>(), ["o/a"]);
    }

    #[test]
    fn only_terminal_states_are_finished() {
        for status in [RunStatus::Success, RunStatus::Failure, RunStatus::Cancelled, RunStatus::Skipped] {
            assert!(status.is_finished(), "{status:?}");
        }
        for status in [RunStatus::Waiting, RunStatus::Running, RunStatus::Blocked, RunStatus::Unknown] {
            assert!(!status.is_finished(), "{status:?}");
        }
    }

    #[test]
    fn an_empty_listing_is_not_an_error() {
        assert!(parse_runs(r#"{"total_count":0,"workflow_runs":[]}"#).unwrap().is_empty());
        assert!(parse_runs("not json").is_err());
    }
}
