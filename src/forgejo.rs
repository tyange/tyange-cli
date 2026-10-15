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
pub struct ActionRun {
    pub status: RunStatus,
    #[serde(default)]
    pub title: String,
    #[serde(default, alias = "trigger_event")]
    pub event: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub html_url: String,
    /// 서버 버전에 따라 필드가 없을 수 있다.
    #[serde(default, alias = "head_sha")]
    pub commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListRunsResponse {
    #[serde(default)]
    workflow_runs: Vec<ActionRun>,
}

pub fn parse_runs(body: &str) -> Result<Vec<ActionRun>, AppError> {
    Ok(serde_json::from_str::<ListRunsResponse>(body)?.workflow_runs)
}

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

    /// 최근 실행 `limit`개(최신이 앞).
    pub fn recent_runs(
        &self,
        owner: &str,
        repo: &str,
        limit: u32,
    ) -> Result<Vec<ActionRun>, AppError> {
        let response = self
            .http
            .get(format!(
                "{}/repos/{owner}/{repo}/actions/runs?limit={limit}",
                self.base
            ))
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
        parse_runs(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_run_list_and_tolerates_unknown_statuses() {
        let runs = parse_runs(
            r#"{"total_count":2,"workflow_runs":[
                {"status":"running","title":"feat: x","event":"push","updated":"2026-10-15T04:00:00Z","html_url":"https://git/x","commit_sha":"abc"},
                {"status":"something-new","title":"old","trigger_event":"pull_request"}
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
