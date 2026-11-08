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
    /// 데스크톱 컨트롤 타워(tytower)의 개요·브레인·NetFile을 다룬다. tytower가 실행 중이어야 한다.
    Tower {
        #[command(subcommand)]
        command: TowerCommand,
    },
}

#[derive(Subcommand)]
pub enum TowerCommand {
    /// tytower가 떠 있는지, 토큰이 맞는지 확인한다.
    Status,
    /// 개요(Work·피드·관심사 소식). 마지막으로 받은 값을 보여 준다.
    Overview {
        /// 지금 다시 받아 온 뒤 보여 준다(끝날 때까지 기다린다).
        #[arg(long)]
        refresh: bool,
        /// 한 출처만 보여 준다.
        #[arg(long, value_parser = ["work", "geeknews", "hn", "trending", "tyange", "repos", "personal"])]
        source: Option<String>,
    },
    /// 세컨드 브레인 노트와 보드(화이트보드)를 본다.
    Brain {
        #[command(subcommand)]
        command: BrainCommand,
    },
    /// NetFile 검색과 파일 받기. 로그인 값은 tytower가 가지고 있다(여기서 입력하지 않는다).
    Netfile {
        #[command(subcommand)]
        command: NetfileCommand,
    },
}

#[derive(Subcommand)]
pub enum BrainCommand {
    /// 노트 목록(경로순).
    List,
    /// 노트 하나(본문·링크·되짚는 노트). id, 제목, 또는 `폴더/제목` 경로로 찾는다.
    Show { note: String },
    /// 제목과 본문에서 찾는다(대소문자 무시).
    Search {
        query: String,
        /// 최대 개수(1~200).
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u64).range(1..=200))]
        limit: u64,
    },
    /// 노트 안의 이미지 파일 하나를 저장한다. 파일 id는 `brain show`의 링크에 나온다.
    File {
        id: String,
        /// 저장할 경로. 이미 있는 파일은 덮어쓰지 않는다.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// 브레인 git 동기화를 시작하고 끝날 때까지 기다린다.
    Sync,
    /// 보드(화이트보드: 노트 카드를 올려 둔 캔버스)를 본다. 카드 배치는 화면에서만 한다.
    Boards {
        #[command(subcommand)]
        command: BoardsCommand,
    },
}

#[derive(Subcommand)]
pub enum BoardsCommand {
    /// 빈 보드를 만든다. 같은 id가 있으면 아무것도 바꾸지 않고 알린다.
    Create {
        /// 새 보드 id(영문 소문자·숫자·하이픈, 첫 글자는 소문자나 숫자, 40자 이하).
        id: String,
        /// 보드 이름(공백을 뺀 뒤 1~80자).
        name: String,
    },
    /// 보드 목록(최근 수정순).
    List,
    /// 보드 하나: 카드, 연결선, 화면 위치. id는 영문 소문자·숫자·하이픈이다.
    Show {
        /// 보드 id(영문 소문자·숫자·하이픈, 첫 글자는 소문자나 숫자, 40자 이하).
        id: String,
    },
    /// 보드 이름을 바꾼다. 마지막으로 본 revision과 맞아야 저장되고, 그사이 바뀌었으면 다시 하라고 알린다.
    Rename {
        /// 보드 id(`show`와 같은 규칙).
        id: String,
        /// 새 이름(공백을 뺀 뒤 1~80자).
        name: String,
    },
    /// 보드를 지운다. 되돌릴 수 없어서 `--yes`가 있어야 한다.
    Delete {
        /// 보드 id(`show`와 같은 규칙).
        id: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum NetfileCommand {
    /// 로그인 값이 있는지(값은 보이지 않는다), 다운로드 폴더, 작업 수.
    Status,
    /// 파일·폴더 검색. 필터 이름은 `nf search`와 같다.
    Search {
        keyword: String,
        /// 결과 안에서 이 단어가 있는 것만.
        #[arg(long = "in", conflicts_with = "exclude")]
        include: Option<String>,
        /// 결과 안에서 이 단어가 없는 것만.
        #[arg(long = "not")]
        exclude: Option<String>,
        /// 종류: video, image, doc.
        #[arg(long = "type", value_parser = ["video", "image", "doc"])]
        kind: Option<String>,
        /// 기간: 1w, 1m, 6m, 1y.
        #[arg(long, value_parser = ["1w", "1m", "6m", "1y"])]
        period: Option<String>,
        /// 크기: under100m, 100m-1g, 1g-5g, 5g-10g, over10g.
        #[arg(long, value_parser = ["under100m", "100m-1g", "1g-5g", "5g-10g", "over10g"])]
        size: Option<String>,
        /// 범위: all(전체), folder(폴더만), file(파일만).
        #[arg(long, default_value = "all", value_parser = ["all", "folder", "file"])]
        scope: String,
    },
    /// 폴더 안의 항목 목록(폴더 id는 검색 결과에서 나온다).
    Ls { folder_id: String },
    /// 파일을 받기 시작한다(여러 개 가능). 검색이나 목록에서 본 id만 받는다.
    Download {
        #[arg(required = true)]
        entry_ids: Vec<String>,
    },
    /// 받기 작업 목록과 진행 상황.
    Jobs,
    /// 작업을 취소한다(대기 중이면 바로, 받는 중이면 다음 조각에서 멈춘다). 끝난 작업이면 목록에서만 지운다. 받은 파일은 남는다.
    Cancel { job_id: String },
    /// 다 받은 파일을 디스크에서 지운다. 파일 이름과 크기를 먼저 보여 주고, `--yes`가 있어야 지운다.
    Rm {
        job_id: String,
        #[arg(long)]
        yes: bool,
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
    fn every_tower_feature_has_a_command() {
        for args in [
            vec!["tyange", "tower", "status"],
            vec!["tyange", "tower", "overview"],
            vec!["tyange", "tower", "overview", "--refresh", "--source", "hn"],
            vec!["tyange", "tower", "overview", "--source", "personal"],
            vec!["tyange", "tower", "brain", "list"],
            vec!["tyange", "tower", "brain", "show", "글"],
            vec!["tyange", "tower", "brain", "search", "rust", "--limit", "5"],
            vec!["tyange", "tower", "brain", "file", "abc_1", "-o", "pic.png"],
            vec!["tyange", "tower", "brain", "sync"],
            vec!["tyange", "tower", "brain", "boards", "create", "plan", "새 보드"],
            vec!["tyange", "tower", "brain", "boards", "list"],
            vec!["tyange", "tower", "brain", "boards", "show", "plan"],
            vec!["tyange", "tower", "brain", "boards", "rename", "plan", "새 이름"],
            vec!["tyange", "tower", "brain", "boards", "delete", "plan", "--yes"],
            vec!["tyange", "tower", "netfile", "status"],
            vec!["tyange", "tower", "netfile", "search", "회의", "--in", "예산", "--type", "doc", "--scope", "file"],
            vec!["tyange", "tower", "netfile", "search", "rust", "--not", "draft", "--period", "1y", "--size", "over10g"],
            vec!["tyange", "tower", "netfile", "ls", "folder-1"],
            vec!["tyange", "tower", "netfile", "download", "e1", "e2"],
            vec!["tyange", "tower", "netfile", "jobs"],
            vec!["tyange", "tower", "netfile", "cancel", "job-1"],
            vec!["tyange", "tower", "netfile", "rm", "job-1", "--yes"],
        ] {
            assert!(Cli::try_parse_from(&args).is_ok(), "{args:?}");
        }
        assert!(Cli::try_parse_from(["tyange", "tower", "overview", "--source", "nope"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "tower", "netfile", "search", "x", "--type", "gif"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "tower", "netfile", "search", "x", "--in", "a", "--not", "b"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "search", "x", "--limit", "0"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "file", "abc"]).is_err(), "-o는 필수");
        assert!(Cli::try_parse_from(["tyange", "tower", "netfile", "download"]).is_err(), "id가 하나는 있어야 한다");
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "show"]).is_err());
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "boards", "rename", "plan"]).is_err(), "이름은 필수");
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "boards", "create", "plan"]).is_err(), "create도 이름은 필수");
        assert!(Cli::try_parse_from(["tyange", "tower", "brain", "boards"]).is_err(), "하위 명령을 골라야 한다");
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
