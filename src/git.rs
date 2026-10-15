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
    let (host, path) = if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':')?
    } else {
        url.strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))?
            .split_once('/')?
    };
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    if host.is_empty() || owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some(RemoteInfo {
        host: host.to_string(),
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// 현재 디렉터리 리포의 `origin` URL.
pub fn origin_url() -> Result<String, crate::error::AppError> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()?;
    if !output.status.success() {
        return Err(crate::error::AppError::NotAGitRepo);
    }
    let url = String::from_utf8(output.stdout)
        .map_err(|e| crate::error::AppError::Invalid(e.to_string()))?;
    Ok(url.trim().to_string())
}

/// 현재 HEAD 커밋 해시. 리포가 아니면 `None`.
pub fn head_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
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
