use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// tyange 개인 인프라를 다루는 CLI.
///
/// 서브커맨드가 계속 늘어나는 구조다. 새 마찰이 생기면 여기에 하나씩 추가한다.
#[derive(Parser)]
#[command(name = "tyange", version, about = "tyange 개인 인프라 CLI")]
pub struct Cli {
    /// 사람이 읽는 표 대신 JSON으로 출력한다(스크립트·Claude용).
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 현재 디렉터리 리포의 Forgejo Actions 최근 실행 상태를 본다.
    Ci {
        /// 현재 HEAD 커밋의 실행이 끝날 때까지 지켜본다. 성공이면 0, 실패·취소면 1로 끝난다.
        #[arg(long)]
        watch: bool,
        /// --watch가 기다릴 최대 시간(분).
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
    /// tyange-cms-api(블로그·lovelog 글)를 다룬다.
    Cms {
        #[command(subcommand)]
        command: CmsCommand,
    },
    /// CMS 로그인 방식을 관리한다(API 키).
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// API 키를 확인하고 저장한다. 키는 CMS(/api-keys)에서 만든다. 표준 입력이나 입력창(화면에 안 보임)으로 받는다.
    Login {
        /// 저장돼 있던 ID/비밀번호를 지우지 않는다(기본은 키를 저장하면서 지운다).
        #[arg(long)]
        keep_password: bool,
    },
    /// 지금 어떤 방식으로 로그인하는지, 서버가 받아 주는지 확인한다.
    Status,
    /// 저장된 API 키를 지운다.
    Logout,
}

#[derive(Subcommand)]
pub enum CmsCommand {
    /// 글 목록(draft 포함, 최신순).
    Posts {
        /// draft 또는 published만.
        #[arg(long, value_parser = ["draft", "published"])]
        status: Option<String>,
        /// 이 태그가 붙은 글만(이름 기준).
        #[arg(long)]
        tag: Option<String>,
        /// 최대 개수.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// 글 하나(본문 포함).
    Get { id: String },
    /// 마크다운 파일로 새 글을 만든다. 기본은 draft다.
    New {
        /// 본문 파일. `-`면 표준 입력.
        #[arg(long)]
        file: String,
        #[arg(long, default_value = "")]
        title: String,
        #[arg(long, default_value = "")]
        description: String,
        /// `category/name` 또는 `name`(카테고리 general). 여러 번 쓸 수 있다.
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long, default_value = "draft", value_parser = ["draft", "published"])]
        status: String,
        /// 발행일(YYYY-MM-DD). 기본은 오늘(한국 시각).
        #[arg(long)]
        date: Option<String>,
    },
    /// 글을 고친다. 준 항목만 바뀐다. `--tag`를 주면 태그 전체를 교체한다.
    Update {
        id: String,
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long, value_parser = ["draft", "published"])]
        status: Option<String>,
        #[arg(long)]
        date: Option<String>,
    },
    /// 글을 published로 바꾼다.
    Publish { id: String },
    /// 글을 draft로 돌린다.
    Draft { id: String },
    /// 글을 지운다. 되돌릴 수 없어서 `--yes`가 있어야 한다.
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    /// Craft 폴더를 지금 바로 한 번 동기화한다.
    Sync,
    /// Craft 동기화가 켜져 있는지와 마지막 결과.
    SyncStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_command_tree_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn auth_subcommands_parse() {
        assert!(Cli::try_parse_from(["tyange", "auth", "status"]).is_ok());
        assert!(Cli::try_parse_from(["tyange", "auth", "login", "--keep-password"]).is_ok());
        assert!(Cli::try_parse_from(["tyange", "auth", "logout"]).is_ok());
    }

    #[test]
    fn json_is_accepted_after_the_subcommand() {
        let cli = Cli::try_parse_from(["tyange", "cms", "posts", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn status_values_are_validated() {
        assert!(Cli::try_parse_from(["tyange", "cms", "posts", "--status", "oops"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "cms", "posts", "--status", "draft"]).is_ok());
    }
}
