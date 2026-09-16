/// 1단계 — git remote URL을 파싱해서 (host, owner, repo)를 뽑아낸다.
///
/// `tyange ci`가 "지금 이 디렉터리가 어느 Forgejo/GitHub 리포인지"를 알아내는 데 쓴다.
/// `git config --get remote.origin.url`의 출력이 이 함수의 입력이 될 것이다.
///
/// 두 가지 형태를 모두 처리해야 한다:
/// - SSH: `git@git.tyange.com:tyange-labs/palim.git`
/// - HTTPS: `https://github.com/tyange/inert-api.git`
///
/// `.git` 접미사는 있을 수도 없을 수도 있다.
#[derive(Debug, PartialEq, Eq)]
pub struct RemoteInfo {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

pub fn parse_remote_url(url: &str) -> Option<RemoteInfo> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_form() {
        let info = parse_remote_url("git@git.tyange.com:tyange-labs/palim.git").unwrap();
        assert_eq!(info.host, "git.tyange.com");
        assert_eq!(info.owner, "tyange-labs");
        assert_eq!(info.repo, "palim");
    }

    #[test]
    fn parses_https_form() {
        let info = parse_remote_url("https://github.com/tyange/inert-api.git").unwrap();
        assert_eq!(info.host, "github.com");
        assert_eq!(info.owner, "tyange");
        assert_eq!(info.repo, "inert-api");
    }

    #[test]
    fn handles_missing_dot_git_suffix() {
        let info = parse_remote_url("https://github.com/tyange/inert-api").unwrap();
        assert_eq!(info.repo, "inert-api");
    }

    #[test]
    fn rejects_garbage_input() {
        assert!(parse_remote_url("not a url at all").is_none());
    }
}
