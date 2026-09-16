use std::fmt;

/// 이 CLI 전체가 쓰는 에러 타입.
///
/// 새 서브커맨드를 추가할 때마다 여기에 필요한 variant를 늘려간다.
/// (0단계: 아직 아무도 안 씀. 1단계에서 `NotAGitRepo`, `Network`, `Api` 등을 추가한다.)
#[derive(Debug)]
pub enum AppError {
    /// 임시 placeholder. 1단계에서 실제 variant로 교체한다.
    Todo(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Todo(msg) => write!(f, "todo: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}
