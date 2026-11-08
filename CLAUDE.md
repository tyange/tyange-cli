# tyange-cli (`tyange`)

tyange 개인 인프라(Forgejo, tyange-cms-api)와 데스크톱 컨트롤 타워(tytower)를 다루는 Rust CLI. cargo 사용. 바이너리 이름은 `tyange`.
기능 대응표(tytower의 어떤 화면·API가 어떤 명령인지)는 `docs/tower-parity.md`.
`cargo install --path . --locked`로 `~/.cargo/bin/tyange`에 설치한다 — **코드를 고치면 다시 설치해야 반영된다.**

## Claude가 이걸 쓰는 방식 (중요)

**CMS 글 작업·배포 확인은 직접 HTTP 요청을 짜거나 브라우저를 조작하지 말고 이 CLI를 쓴다.**
로그인·인증 헤더·응답 봉투 같은 세부 사항을 CLI가 안에서 처리한다. 결과를 읽을 때는 항상 `--json`을 붙인다.

```bash
# 글
tyange cms posts [--status draft|published] [--tag lovelog] [--category dev] [--search 글자] [--limit 20] [--json]
tyange cms get <id>
tyange cms new --file <md|-> [--tag lovelog|category/name]… [--title ..] [--status draft|published] [--date YYYY-MM-DD]
tyange cms update <id> [--file md|-] [--title ..] [--tag ..]… [--status ..] [--date ..]
tyange cms publish <id>       # published로
tyange cms draft <id>         # draft로 (삭제 대신 이걸 먼저 쓴다)
tyange cms delete <id> --yes  # 되돌릴 수 없다. --yes 없이는 거부한다
tyange cms tags               # 카테고리별 태그 목록
tyange cms image upload <파일> [--post-id <글 id>] [--type in_post|portfolio_icon]   # 주소와 ![](…)를 알려 준다
# 포트폴리오 (섹션: meta, identity, intro, featured_projects, career, portfolio_pdf)
tyange cms portfolio get                         # content만 JSON으로 → 고쳐서 put에 그대로 넣는다
tyange cms portfolio put <파일.json>             # 전체 저장(최초 생성용)
tyange cms portfolio delete --yes
tyange cms portfolio section get <섹션>          # PDF 포트폴리오 데이터는 portfolio_pdf 섹션
tyange cms portfolio section set <섹션> <파일.json>
# 운영
tyange cms guests             # lovelog 청첩장 하객 명단 + 시간대별 인원
tyange cms sync               # Craft 폴더를 지금 바로 동기화 (POST /admin/craft/sync)
tyange cms sync-status        # 동기화가 켜져 있는지, 마지막 결과
tyange cms keys list|create --name .. [--role admin|user] [--save]|revoke <id>   # ID/비밀번호 로그인 필요
# 로그인·배포
tyange auth login [--keep-password]   # API 키를 확인한 뒤 저장 (입력은 화면에 안 보임, 파이프도 가능)
tyange auth status                    # 어떤 방식으로 로그인하는지 + 서버가 받아 주는지
tyange auth logout                    # 저장된 키 삭제 (서버의 키는 CMS /api-keys에서 폐기)
tyange ci [--watch] [--timeout 120]   # 현재 리포의 Forgejo Actions
tyange ci --all [--recent]            # 저장소 폴더 밖에서도: 접근 가능한 모든 저장소에서 지금 돌고 있는 CI (--recent는 저장소마다 마지막 실행)
# tytower (실행 중이어야 한다. 아래 "tytower" 절)
tyange tower status                   # 연결과 토큰 확인
tyange tower overview [--refresh] [--source work|geeknews|hn|trending|tyange|repos|personal]
tyange tower brain list | show <id|제목|폴더/제목> | search <검색어> [--limit n] | file <파일 id> -o <경로> | sync
tyange tower brain boards list | show <id> | rename <id> <이름> | delete <id> --yes   # 보드. 카드 배치·선·화면 위치는 화면에서만(GUI 전용)
tyange tower netfile status | search <검색어> [--in 단어|--not 단어] [--type video|image|doc] [--period 1w|1m|6m|1y] [--size ..] [--scope all|folder|file]
tyange tower netfile ls <폴더 id> | download <항목 id>... | jobs | cancel <작업 id> | rm <작업 id> --yes
```

- **`tyange ci --watch`는 푸시 직후 배포를 기다릴 때 쓴다.** 현재 HEAD 커밋의 실행만 지켜보고, 성공이면 0·실패/취소면 1로 끝난다.
  오래 걸리니(cms-api는 수십 분) 백그라운드로 돌리고 알림을 받는다. HEAD의 실행이 3분 안에 안 나타나면 에러로 끝난다.
- 글을 **발행(`publish`, `new --status published`)하거나 지우는 건 사용자 확인을 받고** 한다. 시험할 때는 draft로 만들고 `delete --yes`로 치운다.
- `cms update`는 서버가 PUT으로 전체 필드를 요구해서 **먼저 글을 읽어 온 뒤 바꿀 항목만 덮어 보낸다**(CLI가 한다).

## tytower (`tyange tower`)

tytower는 이 PC에서 도는 로컬 HTTP 서버다(`127.0.0.1:47820`, `TYTOWER_PORT`). **Overview·Brain·NetFile 명령은 그 HTTP API를 부를 뿐 로직을 다시 만들지 않는다.**
CMS 명령은 예외로 cms-api를 직접 부른다(위 CMS 절). 구현은 `src/tower.rs`(클라이언트) + `src/commands/tower.rs`(상태·개요) + `brain.rs` + `netfile.rs`.

- 토큰은 `Authorization: Bearer <토큰>`으로만 나간다. **토큰은 어떤 출력·에러 문구·`Debug`에도 없다.** 상태 출력도 "설정" 또는 "토큰 파일 경로"만 보여 준다.
- 토큰 찾는 순서: `TYANGE_TOWER_TOKEN`(환경 변수, 그다음 env 파일) → **localhost일 때만** 토큰 파일. 원격 주소에서는 토큰 파일을 읽지 않는다.
  토큰 파일 위치: `TYTOWER_CONFIG_DIR`가 있으면 그 폴더의 `tower-token`, 없으면 Windows `%APPDATA%\com.tyange.tytower\tower-token`(macOS `~/Library/Application Support/…`, Linux `$XDG_CONFIG_HOME` 또는 `~/.config/…`).
  그래서 이 PC에서는 설정 없이 동작한다.
- tytower가 꺼져 있으면 `tytower가 실행 중이 아닙니다 … TYTOWER_HEADLESS=1 tytower` 안내로 끝난다(exit 1).
- `overview --refresh`와 `brain sync`는 시작하고 **끝날 때까지 기다린다**(`wait_for`, 제한 시간 180초·300초). 이미 돌고 있으면(409) 기다리기만 한다.
- `brain show <이름>`은 제목·경로(확장자 없이도)로 찾는다. 여러 개가 맞으면 목록을 보여 주고 id로 고르게 한다.
- 보드(`brain boards`)는 타워 서버가 저장한다(노트 폴더에 쓰지 않는다). 보드 id는 `[a-z0-9][a-z0-9-]{0,39}`로 **보내기 전에** 검사하고, 경로는 검사를 통과한 값으로만 만든다.
  이름 바꾸기는 GET으로 읽은 `revision`을 실어 PUT한다. 409(그사이 바뀜)면 저장하지 않고 다시 하라고 알린다. 없는 보드는 PUT으로 만들지 않는다(GET이 404면 중단).
- `brain file`은 이미 있는 파일을 **덮어쓰지 않는다**.
- `netfile rm <작업 id>`는 목록에서 작업을 찾고 상태가 `done`인지 확인한 뒤, **파일 이름과 크기를 보여 주고 `--yes`가 있어야** 지운다. 없으면 DELETE를 보내지 않는다. 서버도 다운로드 폴더 안의 일반 파일만 지운다.
- `netfile` 검색 필터 이름은 `nf search`와 같다(`--in`/`--not`, `--type`, `--period`, `--size`, `--scope`). 검색 본문은 tytower 웹 화면이 보내는 모양과 같다(빠진 필터는 뺀다).
- NetFile 로그인 값은 CLI가 다루지 않는다. `netfile status`는 "있다/없다"와 출처만 보여 준다.
- 테스트 환경에서는 `TYTOWER_NETFILE_DIR`를 임시 폴더로 바꿔서 실제 Downloads 폴더를 만들지 않게 한다.

## 설정

환경변수가 우선이고, 없으면 `~/.config/tyange-cli/env`(`KEY=VALUE` 줄)에서 읽는다. **값은 어떤 출력에도 내지 않는다**(`Config`의 `Debug`도 키 이름만).
채팅에 비밀번호·토큰을 쓰게 하지 말고 사용자가 파일을 직접 만들게 한다.

| 키 | 용도 | 기본 |
|---|---|---|
| `TYANGE_CMS_API_KEY` | **CMS API 키(`tyk_…`). 있으면 이걸 쓴다** — 로그인 요청이 없다. `tyange auth login`이 저장한다 | 없음 |
| `TYANGE_CMS_ID`, `TYANGE_CMS_PASSWORD` | 키가 없을 때의 ID/비밀번호 로그인(예전 방식) | 없음 |
| `TYANGE_CMS_API_BASE` | cms-api 주소 | `https://tyange.com/api/cms` |
| `TYANGE_FORGEJO_TOKEN` | `ci`용 Forgejo 토큰(저장소 읽기 권한) | 없음(필수) |
| `TYANGE_FORGEJO_HOST` | `ci --all`이 목록을 가져올 Forgejo 호스트 | `git.tyange.com` |
| `TYANGE_TOWER_URL` | tytower 주소 | `http://127.0.0.1:47820` |
| `TYANGE_TOWER_TOKEN` | tytower 토큰. 없으면 이 PC의 토큰 파일을 읽는다(localhost일 때만) | 없음 |
| `TYTOWER_CONFIG_DIR` | tytower의 설정 폴더(토큰 파일이 여기 있다). tytower와 같은 변수 | 앱 설정 폴더 |

## 구조

```
src/
  main.rs         Config 로드 → 서브커맨드 디스패치. 에러는 stderr에 `error: …`, 종료 코드 1
  cli.rs          clap 정의(--json은 global)
  config.rs       환경변수 + env 파일, 값 가림
  error.rs        AppError (Network는 reqwest::Error::without_url()로 URL을 뺀다, TowerNotRunning은 주소의 원점만)
  git.rs          remote URL 파싱, origin/HEAD 조회
  forgejo.rs      Actions 실행 목록 (RunStatus는 모르는 값을 Unknown으로 받는다)
  cms.rs          cms-api 클라이언트 (login, posts, get, create, update, delete, craft sync)
  tower.rs        tytower HTTP 클라이언트(토큰 찾기, Bearer, 에러 모양). 응답은 Value로 들고 다닌다
  commands/ci.rs  ci, ci --watch, ci --all
  commands/cms.rs cms 서브커맨드 분배 + 글 관련(목록·생성·수정·삭제·동기화) + 출력 형식
  commands/{media,portfolio,guests,keys,auth}.rs   태그·이미지 / 포트폴리오 / 하객 / API 키 / 로그인
  commands/tower.rs  tower 분배, status, overview(--refresh, --source), 기다리기(wait_for), 공용 출력 도우미
  commands/{brain,boards,netfile}.rs  tower brain / tower brain boards / tower netfile (노트 고르기, 파일 쓰기, 보드 이름 바꾸기(revision), rm 규칙, 검색 본문)
  shortform.rs    lovelog 200자 규칙 (CMS·lovelog·cms-api와 같은 값, 함께 고친다)
  prompt.rs       한 줄 입력·숨김 입력(stty -echo)
  timeutil.rs     RFC3339 파싱, 한국 시각, 경과 시간
```

## cms-api와 맞물리는 지점 (바뀌면 여기도 고친다)

- 인증 헤더는 `Authorization: <token>` — **`Bearer`를 붙이지 않는다**(서버 `AuthGuard`가 raw 토큰을 읽는다).
- 응답은 대부분 `{status, data, message}` 봉투인데 **공개 `GET /post/:id`만 봉투 없이 글 그대로**다. `cms::unwrap_data`가 둘 다 받는다.
  (글 자체에 `status` 필드가 있어서 `data`까지 같이 있을 때만 봉투로 본다.)
- 태그는 `{tag, category}`. CLI는 `lovelog`(→ general) 또는 `category/name`으로 받는다.
- 이미지 주소는 서버가 `https://tyange.com/api/cms/images/…`로 돌려준다.

## CMS(tyange-cms) 기능 ↔ CLI 대응

| CMS 기능 | CLI | 비고 |
|---|---|---|
| 포스트 목록·조회·등록·수정·삭제, 상태 전환 | `cms posts/get/new/update/publish/draft/delete` | 에디터(마크다운/리치 텍스트) 대신 파일·표준입력. **lovelog 짧은 글 200자 규칙을 CMS와 똑같이 적용**(초과하면 published 저장 거부, draft는 허용) — `src/shortform.rs` |
| 태그·카테고리 조회와 필터 | `cms tags`, `cms posts --tag/--category/--search` | |
| 이미지 업로드(본문·포트폴리오 아이콘) | `cms image upload` | CMS는 브라우저에서 올리기 전에 압축하지만 CLI는 **압축하지 않는다**(서버 상한 10MB를 먼저 검사) |
| 포트폴리오 편집(소개·경력·프로젝트 등 섹션별 저장) | `cms portfolio get/put/delete`, `section get/set` | 섹션별 입력 폼 대신 **JSON 파일**. 서버가 섹션별 스키마로 검증한다 |
| PDF 포트폴리오 편집 | `cms portfolio section get/set portfolio_pdf` | **PDF 파일 생성은 못 한다** — 브라우저 인쇄(`/portfolio-pdf/print`)라서 |
| 하객 명단·시간대별 인원 | `cms guests` | `VISIT_HOURS`는 lovelog·cms-api·CMS 화면과 같은 값(함께 고친다) |
| API 키 발급·폐기 | `cms keys …` | **키로는 키를 관리할 수 없다**(서버가 막음) → ID/비밀번호로 로그인한다. 설정에 없으면 터미널에서 묻는다(저장 안 함) |
| Craft 동기화 | `cms sync`, `cms sync-status` | |
| 로그인 | `auth login/status/logout` | |

CMS에 있지만 CLI에 **없는** 것: 에디터 UI와 자동 저장(해당 없음), 이미지 클라이언트 압축, PDF 파일 생성, 포트폴리오 폼 UI.
cms-api에는 있지만 CMS 화면이 안 쓰는 `/me`·`/me/profile`·`/admin/add-user`(계정 관리)도 CLI에 없다.

## 로그인 방식 (키 우선)

`Cms::connect`가 `TYANGE_CMS_API_KEY`가 있으면 `Authorization: Bearer <키>`로 바로 요청하고(로그인 호출 없음), 없으면 ID/비밀번호로 `/login`해서 받은 JWT를 쓴다
(JWT는 `Bearer` 없이 토큰만 — 서버가 옛 방식도 받는다). 키는 CMS(`/api-keys` 화면, 관리자)에서 만들고 **admin 권한**이어야 한다(`auth status`가 `/admin/craft/status`로 확인).
`auth login`은 **서버가 받아 주는 키일 때만** 저장하고(틀린 키로 기존 설정을 덮어쓰지 않는다), 기본으로 저장돼 있던 ID/비밀번호를 지운다(`--keep-password`로 남김).
env 파일은 항상 `0600`. **키는 채팅에 쓰게 하지 말고 `tyange auth login`을 사용자가 직접 실행하게 한다.**

## 도구 (`tools/`)

- **`tools/extract-ink/extract-ink.ts`** — 손글씨 사진에서 글씨만 남기고 배경은 투명하게 만든다(종이 질감·그림자·바인더가 사라진다). Bun + `sharp`.
  `cd tools/extract-ink && bun install && bun extract-ink.ts <사진> <결과.png> [x,y,w,h]` (`x,y,w,h`는 글씨 영역 자르기).
  종이 밝기를 흐림으로 추정해 "종이보다 얼마나 어두운가"를 투명도로 바꾸고 색은 잉크색으로 통일한다. 결과 PNG를 Craft `CMS` 폴더 문서에 넣으면 cms-api가 lovelog 글로 올린다
  (cms-api의 Craft 동기화는 사진 보정을 하지 않는다). lovelog의 종이 카드(밝은 바탕)에서는 투명 PNG가 잘 보이지만 어두운 바탕에서는 글씨가 안 보인다.

## 규칙

- HTTP는 `reqwest::blocking`(+`rustls`, OpenSSL 불필요). GUIDE.md의 tokio 전환은 아직 안 했다 — `--watch`가 단순 폴링 루프라 필요가 없었다.
- 새 서브커맨드는 `cli.rs`에 정의 → `commands/`에 구현 → **순수 함수(파싱·선택·형식)는 테스트**한다.
- 네트워크를 거치는 명령은 `tests/tower_cli.rs`처럼 **흉내 낸 서버(std `TcpListener`, `tests/support/mod.rs`)에 실제 바이너리를 붙여서** 테스트한다(외부 크레이트 없이). 요청 헤더·본문·경로도 검사한다.
- 사람이 읽는 출력과 `--json` 출력을 둘 다 지원한다. 출력에 비밀값을 섞지 않는다.
- 삭제처럼 되돌릴 수 없는 동작은 명시적 플래그(`--yes`)를 요구한다.
- Windows 빌드: `config.rs`의 0600 권한 설정은 `cfg(unix)`에서만 한다(Windows에는 모드 비트가 없다). 그 밖에 OS별 코드를 넣을 때도 같은 식으로 가린다.

## Forgejo API에서 확인한 것 (2026-10-15, 실제 응답)

- 실행 목록 `GET /api/v1/repos/{owner}/{repo}/actions/runs?limit=N` → `{ total_count, workflow_runs: [...] }`. 실행에는 `status`, `title`, `prettyref`(브랜치),
  `commit_sha`, `started`/`stopped`/`created`/`updated`(RFC3339, `Z`), `duration`(나노초), `html_url`, `repository.full_name`이 있다.
  **`event`와 `trigger_event`가 둘 다 온다 — `event`에 `alias = "trigger_event"`를 걸면 "duplicate field" 에러가 난다**(처음 구현에서 실제로 났고, 실제 응답을 줄인 테스트로 고정했다).
- `?status=running` 필터는 저장소 단위로 동작한다. 그러나 **조직·관리자·사용자 단위 실행 목록 API는 없다**(`/orgs/{org}/actions/runs` 등 전부 404) — 그래서 `ci --all`은
  `GET /repos/search`로 저장소를 모은 뒤(`has_actions && !archived`, 페이지 50씩) **저장소마다 최근 10개를 병렬(6개 스레드)로** 받아 CLI에서 거른다.
  "돌고 있음"은 `waiting | running | blocked`(`RunStatus::is_in_progress`).
- 인증은 `Authorization: token <토큰>`(Bearer 아님).

## 검증 상태

- `cargo test` (Windows, Rust 1.95): 단위 테스트 기존 65개 + 새로 추가한 것, 통합 테스트 25개(`tests/tower_cli.rs`). 전부 통과.
  M5 이전 기록은 65개 통과다.
- 실제 tytower 헤드리스(포트 47890, 임시 설정·브레인 폴더)에 대해 `tower status`, `overview`, `brain list/show/search`, `netfile status`를 실행해 확인했다(운영 cms-api·실제 NetFile 로그인 값은 쓰지 않았다).
- `tower netfile rm`·`download`·`cancel`은 흉내 낸 서버로만 확인했다(실제 NetFile에 요청하지 않는다).
- **읽기 명령은 운영 서버에 실제로 호출해 확인**했다(`tags`, `posts` 필터, `portfolio get`/`section get`, `guests`, `ci`, `ci --all`, `auth status`).
- **쓰기 명령(`portfolio put/section set/delete`, `image upload`, `keys create/revoke`, 글 규칙)은 운영이 아니라 임시 DB로 띄운 로컬 cms-api로** 끝까지 확인했다
  (운영 포트폴리오나 키를 시험 삼아 건드리지 않으려고). 이때 `portfolio delete`가 **빈 본문 200 응답을 JSON 오류로 오해**하던 버그를 찾아 고쳤다(`send_ok`/`status_error`, 테스트로 고정).
- **터미널 입력 경로는 자동 테스트하지 못했다**: `auth login`·`keys`의 숨김 입력(`stty -echo`)과 ID/비밀번호 묻기는 TTY가 있어야 해서 파이프로만 확인했다.
- `ci --all`에서 "돌고 있는 CI가 있을 때"의 줄은 실제 서버로는 못 봤다(확인 시점에 도는 CI가 없었다).
