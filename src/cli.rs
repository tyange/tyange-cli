use clap::{Parser, Subcommand};

/// tyange 개인 인프라를 다루는 CLI.
///
/// 서브커맨드가 계속 늘어나는 구조다. 새 마찰이 생기면 여기에 하나씩 추가한다.
#[derive(Parser)]
#[command(name = "tyange", version, about = "tyange 개인 인프라 CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 현재 디렉터리 리포의 Forgejo Actions 최근 실행 상태를 본다.
    Ci {
        /// 실행이 끝날 때까지 폴링하며 지켜본다.
        #[arg(long)]
        watch: bool,
    },
    // 1단계 이후 여기에 추가:
    // Repos,   // 여러 리포 상태판
    // Publish { path: PathBuf },  // 마크다운 발행
    // New { name: String, template: String },  // 리포 스캐폴딩
}
