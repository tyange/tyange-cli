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
        /// 저장소 폴더 밖에서도: 접근할 수 있는 모든 저장소에서 지금 돌고 있는 CI를 모아 본다.
        #[arg(long, conflicts_with = "watch")]
        all: bool,
        /// --all과 함께: 돌고 있는 것 대신 저장소마다 마지막 실행을 본다.
        #[arg(long, requires = "all")]
        recent: bool,
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
        /// 이 카테고리의 태그가 붙은 글만(예: general, dev).
        #[arg(long)]
        category: Option<String>,
        /// 제목이나 본문 앞부분(300자)에 이 글자가 있는 글만(대소문자 무시).
        #[arg(long)]
        search: Option<String>,
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
        /// 새 본문 파일. `-`면 표준 입력.
        #[arg(long)]
        file: Option<String>,
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
    /// 카테고리별 태그 목록.
    Tags,
    /// 이미지를 올린다(글 본문·포트폴리오 아이콘).
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
    /// 포트폴리오(소개·경력·프로젝트·PDF 포트폴리오)를 읽고 고친다.
    Portfolio {
        #[command(subcommand)]
        command: PortfolioCommand,
    },
    /// lovelog 청첩장 하객 명단과 시간대별 인원.
    Guests,
    /// API 키를 만들고 폐기한다. **ID/비밀번호 로그인이 필요하다**(API 키로는 안 된다).
    Keys {
        #[command(subcommand)]
        command: KeysCommand,
    },
}

#[derive(Subcommand)]
pub enum ImageCommand {
    /// 이미지 파일(png·jpg·gif·webp, 10MB 이하)을 올리고 주소를 알려 준다.
    Upload {
        file: PathBuf,
        /// 이 글에 연결한다(글 id).
        #[arg(long)]
        post_id: Option<String>,
        /// 종류: 글 본문(in_post) 또는 포트폴리오 아이콘(portfolio_icon).
        #[arg(long = "type", default_value = "in_post", value_parser = ["in_post", "portfolio_icon"])]
        kind: String,
    },
}

#[derive(Subcommand)]
pub enum PortfolioCommand {
    /// 포트폴리오 문서(`content`)를 JSON으로 출력한다. 고쳐서 `put`에 그대로 넣을 수 있다.
    Get,
    /// JSON 파일로 포트폴리오 전체를 저장한다(최초 생성용. 평소에는 `section set`).
    Put { file: PathBuf },
    /// 포트폴리오 전체를 지운다. 되돌릴 수 없어서 `--yes`가 있어야 한다.
    Delete {
        #[arg(long)]
        yes: bool,
    },
    /// 섹션 하나를 읽고 쓴다.
    Section {
        #[command(subcommand)]
        command: SectionCommand,
    },
}

#[derive(Subcommand)]
pub enum SectionCommand {
    /// 섹션의 `content`를 JSON으로 출력한다. 키: meta, identity, intro, featured_projects, career, portfolio_pdf.
    Get { key: String },
    /// JSON 파일로 섹션을 저장한다. 서버가 섹션별 스키마로 검증한다.
    Set { key: String, file: PathBuf },
}

#[derive(Subcommand)]
pub enum KeysCommand {
    /// 내 API 키 목록(폐기한 것 포함). 원문은 나오지 않는다.
    List,
    /// 새 키를 만든다. 원문은 **이번 한 번만** 보여 준다.
    Create {
        /// 알아볼 이름(예: "tyange CLI (맥북)").
        #[arg(long)]
        name: String,
        /// admin은 관리 API까지, user는 글·이미지 쓰기만.
        #[arg(long, default_value = "admin", value_parser = ["admin", "user"])]
        role: String,
        /// 만든 키를 이 기기의 설정(~/.config/tyange-cli/env)에 바로 저장한다.
        #[arg(long)]
        save: bool,
    },
    /// 키를 폐기한다(즉시 인증이 안 된다).
    Revoke { id: i64 },
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
    fn ci_all_flags_are_wired_correctly() {
        assert!(Cli::try_parse_from(["tyange", "ci", "--all"]).is_ok());
        assert!(Cli::try_parse_from(["tyange", "ci", "--all", "--recent", "--json"]).is_ok());
        assert!(Cli::try_parse_from(["tyange", "ci", "--recent"]).is_err(), "--recent는 --all이 있어야 한다");
        assert!(Cli::try_parse_from(["tyange", "ci", "--all", "--watch"]).is_err(), "둘은 함께 쓸 수 없다");
    }

    #[test]
    fn every_cms_feature_has_a_command() {
        for args in [
            vec!["tyange", "cms", "posts", "--category", "dev", "--search", "rust"],
            vec!["tyange", "cms", "tags"],
            vec!["tyange", "cms", "image", "upload", "a.png", "--type", "portfolio_icon", "--post-id", "p1"],
            vec!["tyange", "cms", "portfolio", "get"],
            vec!["tyange", "cms", "portfolio", "put", "p.json"],
            vec!["tyange", "cms", "portfolio", "delete", "--yes"],
            vec!["tyange", "cms", "portfolio", "section", "get", "meta"],
            vec!["tyange", "cms", "portfolio", "section", "set", "career", "c.json"],
            vec!["tyange", "cms", "guests"],
            vec!["tyange", "cms", "keys", "list"],
            vec!["tyange", "cms", "keys", "create", "--name", "x", "--role", "user", "--save"],
            vec!["tyange", "cms", "keys", "revoke", "4"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{args:?}");
        }
        assert!(Cli::try_parse_from(["tyange", "cms", "image", "upload", "a.png", "--type", "gif"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "cms", "keys", "create"]).is_err(), "--name은 필수");
        assert!(Cli::try_parse_from(["tyange", "cms", "keys", "revoke", "abc"]).is_err());
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
