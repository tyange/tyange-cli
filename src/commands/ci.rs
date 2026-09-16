use crate::error::AppError;

/// `tyange ci` — 현재 리포의 Forgejo Actions 최근 실행 상태를 본다.
///
/// 1단계에서 채운다. 순서:
/// 1. `git config --get remote.origin.url`을 서브프로세스로 실행해서 remote URL을 얻는다
///    (`std::process::Command`)
/// 2. `crate::git::parse_remote_url`로 host/owner/repo를 뽑는다
/// 3. Forgejo API를 호출해서 최근 워크플로 실행을 가져온다
///    (`GET /api/v1/repos/{owner}/{repo}/actions/tasks`)
///    — 이 단계에서 `reqwest`, `serde`, `tokio`를 Cargo.toml에 추가한다
/// 4. 상태를 사람이 읽기 좋게 출력한다
///
/// `watch`가 true면 완료 상태(success/failure)가 될 때까지 폴링한다 (2단계).
pub fn run(watch: bool) -> Result<(), AppError> {
    todo!()
}
