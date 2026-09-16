/// 1단계에서 채운다.
///
/// Forgejo API 토큰, 베이스 URL 같은 걸 담는다.
/// 환경변수(`TYANGE_FORGEJO_TOKEN` 등)에서 읽어오는 걸 우선 구현하고,
/// 나중에 설정 파일(`~/.config/tyange-cli/config.toml`)로 확장해도 된다.
pub struct Config {
    // pub forgejo_token: String,
    // pub forgejo_base_url: String,
}
