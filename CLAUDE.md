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
tyange ci --all [--recent]            # 저장소 폴더 밖에서도: 접근 가능한 모든 저장소에서 지금 돌고 있는 CI (--recent는 저장소마다 마지막 실행)
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
| `TYANGE_FORGEJO_HOST` | `ci --all`이 목록을 가져올 Forgejo 호스트 | `git.tyange.com` |

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

## 도구 (`tools/`)

- **`tools/extract-ink/extract-ink.ts`** — 손글씨 사진에서 글씨만 남기고 배경은 투명하게 만든다(종이 질감·그림자·바인더가 사라진다). Bun + `sharp`.
  `cd tools/extract-ink && bun install && bun extract-ink.ts <사진> <결과.png> [x,y,w,h]` (`x,y,w,h`는 글씨 영역 자르기).
  종이 밝기를 흐림으로 추정해 "종이보다 얼마나 어두운가"를 투명도로 바꾸고 색은 잉크색으로 통일한다. 결과 PNG를 Craft `CMS` 폴더 문서에 넣으면 cms-api가 lovelog 글로 올린다
  (cms-api의 Craft 동기화는 사진 보정을 하지 않는다). lovelog의 종이 카드(밝은 바탕)에서는 투명 PNG가 잘 보이지만 어두운 바탕에서는 글씨가 안 보인다.

## 규칙

- HTTP는 `reqwest::blocking`(+`rustls`, OpenSSL 불필요). GUIDE.md의 tokio 전환은 아직 안 했다 — `--watch`가 단순 폴링 루프라 필요가 없었다.
- 새 서브커맨드는 `cli.rs`에 정의 → `commands/`에 구현 → **순수 함수(파싱·선택·형식)는 테스트**한다. 네트워크 호출 자체는 테스트하지 않는다.
- 사람이 읽는 출력과 `--json` 출력을 둘 다 지원한다. 출력에 비밀값을 섞지 않는다.
- 삭제처럼 되돌릴 수 없는 동작은 명시적 플래그(`--yes`)를 요구한다.

## Forgejo API에서 확인한 것 (2026-10-15, 실제 응답)

- 실행 목록 `GET /api/v1/repos/{owner}/{repo}/actions/runs?limit=N` → `{ total_count, workflow_runs: [...] }`. 실행에는 `status`, `title`, `prettyref`(브랜치),
  `commit_sha`, `started`/`stopped`/`created`/`updated`(RFC3339, `Z`), `duration`(나노초), `html_url`, `repository.full_name`이 있다.
  **`event`와 `trigger_event`가 둘 다 온다 — `event`에 `alias = "trigger_event"`를 걸면 "duplicate field" 에러가 난다**(처음 구현에서 실제로 났고, 실제 응답을 줄인 테스트로 고정했다).
- `?status=running` 필터는 저장소 단위로 동작한다. 그러나 **조직·관리자·사용자 단위 실행 목록 API는 없다**(`/orgs/{org}/actions/runs` 등 전부 404) — 그래서 `ci --all`은
  `GET /repos/search`로 저장소를 모은 뒤(`has_actions && !archived`, 페이지 50씩) **저장소마다 최근 10개를 병렬(6개 스레드)로** 받아 CLI에서 거른다.
  "돌고 있음"은 `waiting | running | blocked`(`RunStatus::is_in_progress`).
- 인증은 `Authorization: token <토큰>`(Bearer 아님).

## 검증 상태

- `cargo test` 43개 통과. `cms`·`auth`·`ci`·`ci --all` 모두 실제 서버에 호출해 확인했다.
- **`ci --all`에서 "돌고 있는 CI가 있을 때"의 줄은 실제 서버로는 못 봤다**(확인 시점에 도는 CI가 없었다). 그 경로는 실제 응답의 필드로 만든 합성 데이터 테스트로만 확인했다.
