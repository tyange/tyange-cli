# tyange-cli (`tyange`)

tyange 개인 인프라(Forgejo, tyange-cms-api)를 다루는 Rust CLI. cargo 사용. 바이너리 이름은 `tyange`.
`cargo install --path . --locked`로 `~/.cargo/bin/tyange`에 설치한다 — **코드를 고치면 다시 설치해야 반영된다.**

## Claude가 이걸 쓰는 방식 (중요)

**CMS 글 작업·배포 확인은 직접 HTTP 요청을 짜거나 브라우저를 조작하지 말고 이 CLI를 쓴다.**
로그인·인증 헤더·응답 봉투 같은 세부 사항을 CLI가 안에서 처리한다. 결과를 읽을 때는 항상 `--json`을 붙인다.

```bash
tyange cms posts [--status draft|published] [--tag lovelog] [--limit 20] [--json]
tyange cms get <id>
tyange cms new --file <md|-> [--tag lovelog] [--title ..] [--status draft|published] [--date YYYY-MM-DD]
tyange cms update <id> [--file md] [--title ..] [--tag ..] [--status ..] [--date ..]
tyange cms publish <id>       # published로
tyange cms draft <id>         # draft로 (삭제 대신 이걸 먼저 쓴다)
tyange cms delete <id> --yes  # 되돌릴 수 없다. --yes 없이는 거부한다
tyange cms sync               # Craft 폴더를 지금 바로 동기화 (POST /admin/craft/sync)
tyange cms sync-status        # 동기화가 켜져 있는지, 마지막 결과
tyange ci [--watch] [--timeout 120]   # 현재 리포의 Forgejo Actions
tyange auth login [--keep-password]   # API 키를 확인한 뒤 저장 (입력은 화면에 안 보임, 파이프도 가능)
tyange auth status                    # 어떤 방식으로 로그인하는지 + 서버가 받아 주는지
tyange auth logout                    # 저장된 키 삭제 (서버의 키는 CMS /api-keys에서 폐기)
```

- **`tyange ci --watch`는 푸시 직후 배포를 기다릴 때 쓴다.** 현재 HEAD 커밋의 실행만 지켜보고, 성공이면 0·실패/취소면 1로 끝난다.
  오래 걸리니(cms-api는 수십 분) 백그라운드로 돌리고 알림을 받는다. HEAD의 실행이 3분 안에 안 나타나면 에러로 끝난다.
- 글을 **발행(`publish`, `new --status published`)하거나 지우는 건 사용자 확인을 받고** 한다. 시험할 때는 draft로 만들고 `delete --yes`로 치운다.
- `cms update`는 서버가 PUT으로 전체 필드를 요구해서 **먼저 글을 읽어 온 뒤 바꿀 항목만 덮어 보낸다**(CLI가 한다).

## 설정

환경변수가 우선이고, 없으면 `~/.config/tyange-cli/env`(`KEY=VALUE` 줄)에서 읽는다. **값은 어떤 출력에도 내지 않는다**(`Config`의 `Debug`도 키 이름만).
채팅에 비밀번호·토큰을 쓰게 하지 말고 사용자가 파일을 직접 만들게 한다.

| 키 | 용도 | 기본 |
|---|---|---|
| `TYANGE_CMS_API_KEY` | **CMS API 키(`tyk_…`). 있으면 이걸 쓴다** — 로그인 요청이 없다. `tyange auth login`이 저장한다 | 없음 |
| `TYANGE_CMS_ID`, `TYANGE_CMS_PASSWORD` | 키가 없을 때의 ID/비밀번호 로그인(예전 방식) | 없음 |
| `TYANGE_CMS_API_BASE` | cms-api 주소 | `https://tyange.com/api/cms` |
| `TYANGE_FORGEJO_TOKEN` | `ci`용 Forgejo 토큰(저장소 읽기 권한) | 없음(필수) |

## 구조

```
src/
  main.rs         Config 로드 → 서브커맨드 디스패치. 에러는 stderr에 `error: …`, 종료 코드 1
  cli.rs          clap 정의(--json은 global)
  config.rs       환경변수 + env 파일, 값 가림
  error.rs        AppError (Network는 reqwest::Error::without_url()로 URL을 뺀다)
  git.rs          remote URL 파싱, origin/HEAD 조회
  forgejo.rs      Actions 실행 목록 (RunStatus는 모르는 값을 Unknown으로 받는다)
  cms.rs          cms-api 클라이언트 (login, posts, get, create, update, delete, craft sync)
  commands/ci.rs  ci, ci --watch (pick_run: HEAD 커밋의 실행만 고른다)
  commands/cms.rs cms 서브커맨드 + 출력 형식
```

## cms-api와 맞물리는 지점 (바뀌면 여기도 고친다)

- 인증 헤더는 `Authorization: <token>` — **`Bearer`를 붙이지 않는다**(서버 `AuthGuard`가 raw 토큰을 읽는다).
- 응답은 대부분 `{status, data, message}` 봉투인데 **공개 `GET /post/:id`만 봉투 없이 글 그대로**다. `cms::unwrap_data`가 둘 다 받는다.
  (글 자체에 `status` 필드가 있어서 `data`까지 같이 있을 때만 봉투로 본다.)
- 태그는 `{tag, category}`. CLI는 `lovelog`(→ general) 또는 `category/name`으로 받는다.
- 이미지 주소는 서버가 `https://tyange.com/api/cms/images/…`로 돌려준다.

## 로그인 방식 (키 우선)

`Cms::connect`가 `TYANGE_CMS_API_KEY`가 있으면 `Authorization: Bearer <키>`로 바로 요청하고(로그인 호출 없음), 없으면 ID/비밀번호로 `/login`해서 받은 JWT를 쓴다
(JWT는 `Bearer` 없이 토큰만 — 서버가 옛 방식도 받는다). 키는 CMS(`/api-keys` 화면, 관리자)에서 만들고 **admin 권한**이어야 한다(`auth status`가 `/admin/craft/status`로 확인).
`auth login`은 **서버가 받아 주는 키일 때만** 저장하고(틀린 키로 기존 설정을 덮어쓰지 않는다), 기본으로 저장돼 있던 ID/비밀번호를 지운다(`--keep-password`로 남김).
env 파일은 항상 `0600`. **키는 채팅에 쓰게 하지 말고 `tyange auth login`을 사용자가 직접 실행하게 한다.**

## 규칙

- HTTP는 `reqwest::blocking`(+`rustls`, OpenSSL 불필요). GUIDE.md의 tokio 전환은 아직 안 했다 — `--watch`가 단순 폴링 루프라 필요가 없었다.
- 새 서브커맨드는 `cli.rs`에 정의 → `commands/`에 구현 → **순수 함수(파싱·선택·형식)는 테스트**한다. 네트워크 호출 자체는 테스트하지 않는다.
- 사람이 읽는 출력과 `--json` 출력을 둘 다 지원한다. 출력에 비밀값을 섞지 않는다.
- 삭제처럼 되돌릴 수 없는 동작은 명시적 플래그(`--yes`)를 요구한다.

## 검증 상태

- `cargo test` 32개 통과. `auth`(키 발급→login→사용→폐기→거부→logout)는 **로컬 cms-api를 임시 DB로 띄워** 끝까지 확인했다(운영 서버에는 아직 키 기능이 배포되기 전). `cms` 명령은 운영 서버에 실제로 호출해 확인했다(목록·조회·생성·수정·발행/draft·삭제·동기화).
- **`ci`는 실제 Forgejo에 대고 확인하지 못했다.** 토큰이 없어서다. 실행 목록의 `commit_sha`(없으면 `head_sha`) 필드 이름과 응답 모양은
  GUIDE.md의 기록과 Forgejo 문서에 기댄 가정이다 — 처음 돌릴 때 `tyange ci --json`으로 필드를 확인할 것. 커밋 해시 필드가 없으면
  `pick_run`이 가장 최근 실행으로 떨어진다(푸시 직후엔 옛 실행을 볼 수 있다).
