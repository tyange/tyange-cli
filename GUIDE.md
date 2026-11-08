# tyange-cli 진행 가이드

`rust-practice`/`notaredis`와 같은 방식: 시그니처와 문서 주석만 있고 본문은 `todo!()`.
테스트를 통과시키면서 채운다.

---

## 0단계 — CLI 스켈레톤 (완료)

`src/cli.rs`에 clap derive로 `Cli`/`Commands` 정의, `src/main.rs`에서 디스패치.
`cargo run -- --help`, `cargo run -- ci --help`가 이미 동작한다.

---

## 1단계 — `tyange ci` (6~8시간)

### 1-A. remote URL 파싱 (`src/git.rs`, 2시간)

**의존성 추가 없음.** 순수 문자열 파싱만으로 끝난다.

`parse_remote_url`을 채운다. 테스트 4개가 이미 있다 — SSH 형태, HTTPS 형태,
`.git` 없는 형태, 쓰레기 입력.

**접근 힌트:**
- SSH 형태(`git@host:owner/repo.git`)와 HTTPS 형태(`https://host/owner/repo.git`)는
  구분자가 다르다 (`:` vs `/`). `url.strip_prefix("git@")`로 먼저 분기하자.
- `.git` 접미사는 `strip_suffix(".git")`로 있으면 떼고 없으면 그대로 쓴다
  (`unwrap_or(s)` 패턴).
- 못 알아보는 형태는 `None`. `?` 연산자를 `Option`에도 쓸 수 있다는 걸 이 함수에서
  체감하게 된다 (`Option`에서 `?`는 실패 시 즉시 `None` 리턴).

**예상 에러:** `split_once`가 `Option<(&str, &str)>`을 준다. 여러 번 체이닝하다 보면
`&str` 여기저기서 참조 관계가 꼬일 수 있다 — 매번 새 변수에 담고 타입을 확인하며 가자.

### 1-B. 현재 리포의 remote 얻기 (`src/commands/ci.rs`, 1시간)

```rust
std::process::Command::new("git")
    .args(["config", "--get", "remote.origin.url"])
    .output()
```

`output()`은 `Result<Output, io::Error>`를 준다. `stdout`은 `Vec<u8>`이라
`String::from_utf8`로 변환해야 한다 — 이것도 하나의 `Result`.

**여기서 `AppError`를 실제로 확장한다.** `src/error.rs`의 `Todo` variant를 지우고
최소 이 세 개를 만들자:

```rust
pub enum AppError {
    NotAGitRepo,
    UnrecognizedRemote(String),
    Api(String),   // 1-C에서 씀
}
```

`std::process::Command`의 실패, `parse_remote_url`이 `None`을 준 경우를
각각 어떤 variant로 매핑할지 직접 정하자.

### 1-C. Forgejo API 호출 (3~4시간)

**여기서 의존성을 추가한다:**

```bash
cargo add reqwest --features json,blocking
cargo add serde --features derive
cargo add serde_json
```

(async를 2단계에서 다룰 거라, 1단계는 `reqwest::blocking`으로 간단히 간다.
2단계에서 `tokio` + 논블로킹 `reqwest`로 바꿀 것이다 — 그때 "왜 바꾸는가"를
직접 느끼는 게 목적이다.)

**엔드포인트** (실제로 확인함):

```
GET https://{host}/api/v1/repos/{owner}/{repo}/actions/runs?limit=1
Authorization: token {TYANGE_FORGEJO_TOKEN}
```

**토큰 발급:** git.tyange.com → 설정 → 애플리케이션 → 새 토큰 생성
(`repository` 읽기 권한만 있으면 됨). `TYANGE_FORGEJO_TOKEN` 환경변수로 둔다.

**응답 형태:**

```json
{
  "total_count": 42,
  "workflow_runs": [
    {
      "status": "success",
      "title": "...",
      "event": "push",
      "updated": "2026-09-16T...",
      "html_url": "https://git.tyange.com/..."
    }
  ]
}
```

`status`는 `unknown | waiting | running | success | failure | cancelled | skipped | blocked`
중 하나 — **enum으로 모델링하자.** `#[derive(Deserialize)]` + `#[serde(rename_all = "lowercase")]`.

**단계:**
1. `src/forgejo.rs` 새로 만들고 `ActionRun`, `RunStatus`(enum), `ListActionRunResponse` 구조체 정의
2. `get_latest_run(host, owner, repo, token) -> Result<ActionRun, AppError>` 구현
3. `commands::ci::run`에서 1-A, 1-B, 이 함수를 이어붙이고 결과를 출력

**자가 점검:**
- [ ] `git config` 서브프로세스 실패와 "여기가 git 리포가 아님"을 구분해서 처리했다
- [ ] `status` 필드를 문자열로 들고 다니지 않고 enum으로 파싱했다
- [ ] 토큰이 없거나 틀렸을 때 (401) 에러 메시지가 "네트워크 문제"와 구분된다
- [ ] `unwrap()`이 `commands::ci::run` 안에 없다

---

## 2단계 — `tyange ci --watch` (3~4시간)

### 왜 이 단계가 존재하는가

1단계는 한 번 호출하고 끝이다. `--watch`는 상태가 바뀔 때까지 계속 확인해야 한다 —
"루프를 돌리는데, 그 사이에 Ctrl-C도 받아야 한다"는 게 동기 코드로는 지저분해진다.
이게 async가 왜 있는지 체감하는 지점이다.

**의존성 추가:**

```bash
cargo add tokio --features rt-multi-thread,macros,time,signal
```

`reqwest`를 blocking에서 기본(async) 기능으로 바꾼다 (`cargo remove reqwest`
후 `cargo add reqwest --features json`).

**단계:**
1. `#[tokio::main] async fn main()` — `main.rs`를 async로
2. `get_latest_run`을 `async fn`으로, `.send()` 뒤에 `.await`
3. 폴링 루프:
   ```rust
   loop {
       let run = get_latest_run(...).await?;
       print_status(&run);
       if matches!(run.status, RunStatus::Success | RunStatus::Failure) {
           break;
       }
       tokio::time::sleep(Duration::from_secs(5)).await;
   }
   ```
4. **`tokio::select!`로 Ctrl-C 처리 추가.** 폴링 중 `tokio::signal::ctrl_c()`도
   같이 기다리다가, 먼저 오는 쪽을 처리한다.

**자가 점검:**
- [ ] Ctrl-C를 누르면 다음 폴링까지 기다리지 않고 즉시 종료된다
- [ ] `async fn`과 일반 `fn`을 언제 구분해서 쓰는지 설명할 수 있다
- [ ] blocking과 async 버전에서 코드가 어떻게 달라졌는지 비교할 수 있다

---

## 3단계 — `tyange repos` (4~5시간)

24개 로컬 리포를 스캔해서 dirty/unpushed 상태판을 보여준다.

### 단계

1. `~/dev/personal/repositories`를 인자로 받거나 기본값으로 스캔
   (`std::fs::read_dir`, `.git` 디렉터리 있는 것만 필터)
2. 각 리포마다 `git status --porcelain`, `git rev-list --count @{u}..HEAD` 서브프로세스 실행
3. **여러 리포를 동시에 처리한다.** 순차로 하면 24번의 프로세스 스폰이 쌓여 느리다.
   `std::thread::spawn` + `std::sync::mpsc` 채널로 결과를 모으거나,
   이미 2단계에서 tokio를 들였으니 `tokio::task::spawn_blocking`(서브프로세스는
   blocking이라)으로 병렬화해도 된다. **두 방식을 비교해보는 게 이 단계의 핵심.**
4. 출력 포맷: 처음엔 그냥 표. 나중에 `--json` 플래그로 기계가 읽을 형태도 추가해보면
   `trait Formatter { fn format(&self, results: &[RepoStatus]) -> String; }` 같은
   추상화를 만들 이유가 생긴다 (6단계로 미뤄도 됨).

**자가 점검:**
- [ ] 24개 리포를 순차 처리했을 때와 병렬 처리했을 때 시간을 실제로 재봤다
- [ ] 스레드와 `spawn_blocking`의 차이를 설명할 수 있다
- [ ] 리모트가 없는 리포(`indie-film-hub`, `notdir` 등)를 에러 없이 건너뛴다

---

## 4단계 — `tyange publish <file>` (5~7시간)

마크다운 파일 → `tyange-cms-api`로 발행.

### 1. 프론트매터 파싱 — **여기서 lifetime을 의도적으로 넣는다**

```markdown
---
title: 제목
tags: [rust, cli]
---
본문...
```

**먼저 전부 `String`으로 복사하는 버전을 만들어라.** 되면, 그다음 이렇게 바꿔본다:

```rust
pub struct Frontmatter<'a> {
    pub title: &'a str,
    pub raw_yaml: &'a str,
    pub body: &'a str,
}

pub fn split_frontmatter(input: &str) -> Option<Frontmatter<'_>> {
    // "---"로 시작하는 블록을 찾아서 (frontmatter, body)로 나눈다.
    // 전부 input을 가리키는 슬라이스로, 복사 없이.
}
```

**왜 지금 이걸 하는가:** 앞선 두 단계(ci, repos)는 결과를 한 번 쓰고 버리는 값들이라
lifetime을 피해갈 수 있었다. 여기서는 "원본 파일 내용 하나를 여러 조각으로
나눠 담아두는" 구조라 자연스럽게 필요해진다.

**예상 에러:** `Frontmatter<'a>`를 담은 `Vec`을 함수 밖으로 리턴하려는데
원본 `String`이 그 함수 안에서만 살아있으면 컴파일이 안 된다.
**이 에러가 핵심이다.** "빌린 값은 빌려준 사람보다 오래 살 수 없다"를 실제로 겪는다.

### 2. 이미지 경로 치환

본문 안의 `![](./assets/foo.png)` 같은 로컬 경로를 찾아서, 업로드하고,
반환된 URL로 치환한다. `PathBuf`/`&Path` — `String`/`&str`과 똑같은 소유/차용 쌍의
두 번째 사례를 여기서 만난다.

### 3. API 업로드 & 발행

`tyange-cms-api`의 `/post/upload`, `/images/upload` 엔드포인트를 그대로 쓴다
(이미 아는 API라 여기는 빠르게 지나간다). 이미지가 여러 장이면 `tokio::join!` 이나
`futures::future::join_all`로 병렬 업로드해볼 수 있다.

**자가 점검:**
- [ ] `Frontmatter<'a>`가 컴파일 에러 없이 함수 시그니처를 통과했다
- [ ] `&Path`와 `PathBuf`를 언제 쓸지 설명할 수 있다
- [ ] 이미지 업로드 실패 시 발행 자체가 중단되는지 계속되는지 — 설계를 직접 정했다

---

## 5단계(선택) — `tyange new <name> --template <t>`

`tyange-blog`/`tyange-cms`/`tyange-dev`가 공유하는 `.forgejo/workflows/deploy.yml`
패턴을 템플릿화해서 새 리포를 부트스트랩한다. `dialoguer`로 대화형 프롬프트,
템플릿 파일의 `{{name}}` 같은 자리를 치환(직접 문자열 치환으로 시작, 필요하면
`handlebars`/`askama` 크레이트로).

## 6단계(선택) — 아키텍처 정리

`repos`에서 만든 출력과 `ci`의 출력을 공통 `trait Reportable`/`Formatter`로 통합,
`Config`를 파일 기반 설정으로 확장, 통합 테스트 추가.

---

## M5 — tytower 기능을 CLI로 (`tyange tower`)

이 단계는 로드맵 숫자가 없다. tytower(데스크톱 컨트롤 타워)가 가진 기능을 `tyange`에도 같은 모양으로 둔다.
**같은 일을 두 번 구현하지 않는 것**이 핵심이다. Overview·Brain·NetFile은 tytower의 HTTP API를 부르고,
CMS만 cms-api를 직접 부른다.

### 1. 대응표부터 (`docs/tower-parity.md`)

tytower의 라우터·화면·서버 라우트를 한 줄씩 적는다. 각 행에 "지금 명령"을 붙이고, 비어 있는 행(`EMPTY`)을 찾는다.
이 표가 끝나면 할 일이 정해진다. 표를 추측으로 채우지 말고 **`--help`와 실제 응답을 보고** 채운다.

### 2. 설정과 토큰 찾기 (`src/config.rs`, `src/tower.rs`)

- 주소: `TYANGE_TOWER_URL`(기본 `http://127.0.0.1:47820`).
- 토큰: 환경 변수 → env 파일 → (localhost일 때만) 토큰 파일. 토큰 파일 경로는 OS마다 다르다.
  `TYTOWER_CONFIG_DIR`가 있으면 그 폴더를 먼저 본다(tytower와 같은 변수).
- **토큰은 어떤 출력에도 나가지 않는다.** `Debug`도, 에러 문구도.

### 3. HTTP 클라이언트 (`src/tower.rs`)

응답 본문은 `serde_json::Value`로 들고 다닌다. `--json`은 그 값을 그대로 찍고, 사람이 읽는 출력만 타입 구조체로 해석한다.
그래야 tytower가 필드를 더해도 `--json`은 깨지지 않는다.

- 2xx가 아니면 서버의 `{"error","message"}`를 해석한다. 401은 토큰 문제로 안내한다.
- 연결 자체가 안 되면 "tytower가 실행 중이 아닙니다"와 헤드리스 실행법을 알려 준다.

### 4. 명령 그룹 (`src/commands/tower.rs`, `brain.rs`, `netfile.rs`)

- 기다려야 하는 것(갱신·동기화)은 `wait_for`로 한다. 이미 돌고 있으면(409) 에러가 아니라 기다리기다.
- 지우는 것(`netfile rm`)은 **먼저 목록에서 작업을 찾고 상태를 확인한 뒤**, 파일 이름과 크기를 보여 주고 `--yes`를 요구한다.
  `--yes`가 없으면 DELETE를 보내지 않는다. 테스트가 그걸 확인한다.

### 5. 흉내 낸 서버로 테스트 (`tests/tower_cli.rs`, `tests/support/mod.rs`)

외부 크레이트 없이 std `TcpListener`로 tytower를 흉내 낸다. 테스트는 **실제 `tyange` 바이너리**를 실행해서
성공 출력, `--json`, `Authorization` 헤더, 토큰이 출력에 없음, tytower가 꺼져 있을 때 안내, `rm --yes` 규칙을 본다.
마지막으로 실제 tytower를 헤드리스로 띄워서 한 번 돌려 본다(`TYTOWER_HEADLESS=1`, 임시 설정·브레인 폴더, 끝나면 그 PID만 멈춘다).

**자가 점검:**
- [ ] 대응표에 `EMPTY`가 없다
- [ ] 토큰을 바꿔 넣어도 출력에 원래 토큰이 나타나지 않는다(테스트로 확인)
- [ ] `netfile rm`이 `--yes` 없이는 DELETE를 보내지 않는다
- [ ] 원격 주소에서는 토큰 파일을 읽지 않는다
- [ ] tytower를 끄면 같은 명령이 "실행 중이 아닙니다"로 끝난다

---

## 참고

- [Forgejo API 문서](https://git.tyange.com/api/swagger) — 이 리포 자체 인스턴스의 실제 스펙
  (`/repos/{owner}/{repo}/actions/runs` 확인 완료)
- [clap derive 튜토리얼](https://docs.rs/clap/latest/clap/_derive/index.html)
- [tokio 튜토리얼](https://tokio.rs/tokio/tutorial)
