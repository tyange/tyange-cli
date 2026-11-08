use std::fmt;

/// 이 CLI 전체가 쓰는 에러 타입.
///
/// 새 서브커맨드를 추가할 때마다 여기에 필요한 variant를 늘려간다.
#[derive(Debug)]
pub enum AppError {
    /// 여기가 git 리포가 아니거나 `origin` remote가 없다.
    NotAGitRepo,
    /// remote URL을 `host/owner/repo`로 읽지 못했다.
    UnrecognizedRemote(String),
    /// 필요한 설정이 없다. 값은 환경변수 이름.
    MissingConfig(&'static str),
    /// 연결 자체가 안 됐다(DNS, 타임아웃, TLS 등). 링크·토큰이 에러 문구에 새지 않게 URL은 뺀다.
    Network(String),
    /// 서버가 에러로 응답했다.
    Api { status: u16, message: String },
    /// 응답이나 입력을 해석하지 못했다.
    Invalid(String),
    Io(String),
    /// `ci --watch`가 지켜보던 실행이 실패로 끝났다. 종료 코드를 1로 만들려고 에러로 돌려준다.
    RunFailed(String),
    /// tytower(로컬 HTTP 서버)에 연결하지 못했다. 값은 `http://호스트:포트`만 담는다(경로·토큰 없음).
    TowerNotRunning(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotAGitRepo => write!(f, "git 리포가 아니거나 origin remote가 없습니다"),
            AppError::UnrecognizedRemote(url) => write!(f, "remote URL을 읽지 못했습니다: {url}"),
            AppError::MissingConfig(name) => write!(
                f,
                "{name}이(가) 없습니다. 환경변수나 ~/.config/tyange-cli/env에 넣어 주세요"
            ),
            AppError::Network(msg) => write!(f, "네트워크 오류: {msg}"),
            AppError::Api { status: 401, message } => write!(
                f,
                "인증에 실패했습니다(401): {message} — 토큰이나 로그인 정보를 확인하세요"
            ),
            AppError::Api { status, message } => write!(f, "서버 오류({status}): {message}"),
            AppError::Invalid(msg) => write!(f, "{msg}"),
            AppError::Io(msg) => write!(f, "입출력 오류: {msg}"),
            AppError::RunFailed(msg) => write!(f, "{msg}"),
            AppError::TowerNotRunning(origin) => write!(
                f,
                "tytower가 실행 중이 아닙니다({origin}). 데스크톱 앱을 켜거나, 창 없이 띄우려면 TYTOWER_HEADLESS=1 tytower 를 실행하세요"
            ),
        }
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.without_url().to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Invalid(format!("JSON을 읽지 못했습니다: {e}"))
    }
}
