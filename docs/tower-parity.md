# tytower ↔ tyange 기능 대응표 (M5)

tytower(데스크톱 컨트롤 타워)가 하는 일 하나에 행 하나. 기준은 tytower `main`의 라우터(`src/router`), 화면(`src/pages`, `src/components`), 서버 라우트(`src-tauri/src/server.rs`, `netfile.rs`, `overview/http.rs`)와 `docs/api-overview.md`다.

- **M5 전**: 이 브랜치 전에 `tyange`에 있던 명령. `EMPTY`면 그때 없었다.
- **지금**: 이 브랜치 후의 명령. 명령이 있는 행은 명령을 적었다. 부분만 되는 것은 `(부분)`으로 적었다. 명령이 없는 행은 `GUI 전용`뿐이고, 사유를 적었다.
- CMS 행은 cms-api를 직접 부른다(설계). Overview·Brain·NetFile 행은 tytower의 HTTP API를 부른다(`tower` 그룹, `Authorization: Bearer`).

## 요약

| 구분 | 행 수 |
| --- | --- |
| 전체 | 49 |
| M5 전에 명령이 있었다 | 24 (그중 부분 2: 이미지 보기, 로그인) |
| M5 전 `EMPTY` | 19 |
| 설계상 대응이 필요 없다 | 1 (CMS 프록시) |
| M5에서 채웠다 | 18 |
| 명령으로 못 채운 것 | 1 (PDF 파일 내보내기: 데이터 명령만 있다. 사유는 아래) |
| 보드 명령으로 채웠다(행 45~48, 이 브랜치) | 4 |
| GUI 전용(명령 없음, 사유 있음) | 1 (행 49 카드 배치) |

## 대응표

| # | tytower 기능 | tytower 화면·경로 | M5 전 | 지금 (tyange 명령) | 비고 |
| --- | --- | --- | --- | --- | --- |
| 1 | 글 목록(태그 표시) | `/cms/managing-blog` · `GET /admin/posts` | `cms posts` | `cms posts` | |
| 2 | 글 열기(수정 화면 로드) | `/cms/managing-blog/update/:id` · `GET /post/:id` | `cms get` | `cms get <id>` | |
| 3 | 글 작성 | `/cms/managing-blog/create` · `POST /post/upload` | `cms new` | `cms new --file` | |
| 4 | 글 수정 | 에디터 · `PUT /post/update/:id` | `cms update` | `cms update <id>` | |
| 5 | 글 발행 | 에디터 상태 published | `cms publish` | `cms publish <id>` | |
| 6 | 글 draft로 전환 | 에디터 상태 draft | `cms draft` | `cms draft <id>` | |
| 7 | 글 삭제 | 목록 삭제 · `DELETE /post/delete/:id` | `cms delete` | `cms delete <id> --yes` | `--yes` 필요 |
| 8 | 태그·카테고리 목록 | 에디터 · `GET /tags-with-category` | `cms tags` | `cms tags` | |
| 9 | 본문 이미지 업로드 | 에디터 · `POST /upload-image?post_id=` | `cms image upload` | `cms image upload <파일> --post-id <id>` | |
| 10 | 포트폴리오 아이콘 업로드 | `POST /upload-image?image_type=portfolio_icon` | `cms image upload` | `cms image upload <파일> --type portfolio_icon` | |
| 11 | 이미지 보기 | 에디터·목록 · `/api/cms/images/…`(타워 프록시) | `cms get` | `cms get <id>`(부분) | CLI는 이미지를 내려받지 않는다. 본문의 공개 주소만 보여 준다 |
| 12 | 포트폴리오 보기 | `/cms/portfolio` · `GET /portfolio` | `cms portfolio get` | `cms portfolio get` | |
| 13 | 포트폴리오 최초 저장 | `PUT /portfolio/update` | `cms portfolio put` | `cms portfolio put <파일.json>` | 화면은 PUT `/portfolio`가 아니라 `/portfolio/update`를 쓴다. CLI도 같은 경로 |
| 14 | 포트폴리오 삭제 | 편집 화면 삭제 · `DELETE /portfolio` | `cms portfolio delete` | `cms portfolio delete --yes` | `--yes` 필요 |
| 15 | 섹션 읽기(6개) | `GET /portfolio/sections/:key` | `cms portfolio section get` | `cms portfolio section get <섹션>` | |
| 16 | 섹션 저장(6개) | `PUT /portfolio/sections/:key` | `cms portfolio section set` | `cms portfolio section set <섹션> <파일.json>` | |
| 17 | PDF 포트폴리오 편집·저장 | `/cms/portfolio-pdf` · `portfolio_pdf` 섹션 | `cms portfolio section get/set` | `cms portfolio section get/set portfolio_pdf` | |
| 18 | PDF 포트폴리오 이미지 업로드 | `POST /upload-image?post_id=portfolio_pdf` | `cms image upload` | `cms image upload <파일> --post-id portfolio_pdf` | |
| 19 | PDF 파일 내보내기 | `/cms/portfolio-pdf/print` · `window.print()` | `EMPTY` | `cms portfolio section get portfolio_pdf`(데이터만) | **파일은 못 만든다.** 아래 "못 채운 것" |
| 20 | 하객 명단·시간대별 인원 | `/cms/wedding-guests` · `GET /admin/wedding/guests` | `cms guests` | `cms guests` | |
| 21 | API 키 목록 | `/cms/api-keys` · `GET /admin/api-keys` | `cms keys list` | `cms keys list` | |
| 22 | API 키 발급 | `POST /admin/api-keys` | `cms keys create` | `cms keys create --name <이름> [--role admin\|user] [--save]` | |
| 23 | API 키 폐기 | `DELETE /admin/api-keys/:id` | `cms keys revoke` | `cms keys revoke <id>` | |
| 24 | 로그인(ID/비밀번호) | `/cms/login` · `POST /login` | `auth login` | `cms keys …`이 필요할 때 묻는다(저장 안 함). `auth login`은 API 키를 저장(부분) | 키 관리는 ID/비밀번호 로그인이 필요하다 |
| 25 | 관리자 확인(로그인 가드) | `/cms/*` 가드 | `auth status` | `auth status` | |
| 26 | CMS 프록시 | `/api/cms/*` → cms-api | (설계) | 해당 없음 | CLI는 cms-api를 직접 부른다. 위 CMS 행들이 같은 API를 쓴다 |
| 27 | 타워 연결·토큰 확인 | `GET /api/health`, `GET /api/whoami` | `EMPTY` | `tower status` | `--json` 지원 |
| 28 | 토큰 페어링 | 브라우저 `localStorage`(`tower-token`, `src/lib/tower.ts`), 데스크톱 `get_token` | `EMPTY` | `tower status`로 확인. 토큰은 `TYANGE_TOWER_TOKEN` 또는 토큰 파일 | 붙여넣는 화면은 CLI에 없다. 토큰은 설정이나 파일로 준다 |
| 29 | 개요 보기 | `GET /api/overview` (화면은 아직 placeholder: `src/pages/overview.vue`) | `EMPTY` | `tower overview` | |
| 30 | 출처별 보기 | `GET /api/overview`의 `work`, `feeds[id]`, `personal` (화면 미구현) | `EMPTY` | `tower overview --source <출처>` | |
| 31 | 개요 새로 받기 | `POST /api/overview/refresh` (화면 미구현) | `EMPTY` | `tower overview --refresh` | 끝날 때까지 기다린다 |
| 32 | Craft 바로가기 | 개요 응답의 `craft` (화면 미구현) | `EMPTY` | `tower overview`(마지막 줄들) | |
| 33 | 노트 목록 | `GET /api/brain/notes` (`/brain`은 아직 placeholder) | `EMPTY` | `tower brain list` | |
| 34 | 노트 보기(본문·링크·되짚기) | `GET /api/brain/notes/:id` | `EMPTY` | `tower brain show <id\|제목\|폴더/제목>` | 제목이 둘 이상이면 id 목록을 보여 준다 |
| 35 | 노트 검색 | `GET /api/brain/search` | `EMPTY` | `tower brain search <검색어> [--limit n]` | |
| 36 | 노트 이미지 | `GET /api/brain/files/:id` | `EMPTY` | `tower brain file <파일 id> -o <경로>` | 이미 있는 파일은 덮어쓰지 않는다 |
| 37 | 브레인 동기화 | `GET`/`POST /api/brain/sync` | `EMPTY` | `tower brain sync` | 끝날 때까지 기다리고 결과를 보여 준다 |
| 38 | NetFile 상태 | `/netfile`(`src/pages/netfile.vue`) · `GET /api/netfile/status` | `EMPTY` | `tower netfile status` | 로그인 값은 있는지만 보여 준다 |
| 39 | NetFile 검색 | `/netfile` · `POST /api/netfile/search` | `EMPTY` | `tower netfile search <검색어> [--in\|--not] [--type] [--period] [--size] [--scope]` | 필터 이름은 `nf search`와 같다 |
| 40 | 폴더 열기 | `/netfile` · `POST /api/netfile/ls` | `EMPTY` | `tower netfile ls <폴더 id>` | |
| 41 | 받기 시작 | `/netfile` · `POST /api/netfile/downloads` | `EMPTY` | `tower netfile download <항목 id>...` | 검색·목록에서 본 id만 받는다 |
| 42 | 작업 목록·진행 | `NetfileJobs.vue` · `GET /api/netfile/downloads` | `EMPTY` | `tower netfile jobs` | |
| 43 | 받기 취소·목록 지우기 | `NetfileJobs.vue` · `DELETE /api/netfile/downloads/:id` | `EMPTY` | `tower netfile cancel <작업 id>` | 받은 파일은 남는다 |
| 44 | 완료 파일 삭제 | `NetfileJobs.vue` · `DELETE /api/netfile/downloads/:id/file` | `EMPTY` | `tower netfile rm <작업 id> --yes` | 파일 이름과 크기를 먼저 보여 준다. `--yes` 필요 |
| 45 | 보드 목록 | `GET /api/brain/boards`(계약은 tytower 쪽 `boards-contract.md`에 있다. 화면 `/brain`은 아직 placeholder) | `EMPTY` | `tower brain boards list` | 최근 수정순. 보드가 없으면 그렇다고 보여 준다 |
| 46 | 보드 보기(카드·연결선·화면 위치) | `GET /api/brain/boards/:id` | `EMPTY` | `tower brain boards show <id>` | 카드의 노트 내용은 `tower brain show <노트 id>`로 본다 |
| 47 | 보드 이름 바꾸기 | `PUT /api/brain/boards/:id`(읽은 `revision`을 실어 저장) | `EMPTY` | `tower brain boards rename <id> <이름>` | 먼저 읽고 저장한다. 그사이 바뀌었으면(409) 저장하지 않고 다시 하라고 알린다. 없는 보드는 만들지 않는다 |
| 48 | 보드 삭제 | `DELETE /api/brain/boards/:id` | `EMPTY` | `tower brain boards delete <id> --yes` | `--yes` 필요. 없으면 404 안내 |
| 49 | 카드 배치·정렬(카드 옮기기, 선 긋기, 화면 이동·확대) | `/brain` 보드 캔버스(화면 미구현). 저장은 같은 `PUT /api/brain/boards/:id` | `EMPTY` | **GUI 전용**(명령 없음) | 공간 작업이라서다. 위치는 화면을 보면서 끌어서 정하는 일이고, 좌표 숫자를 명령으로 받으면 결과를 볼 수 없어 맞출 수 없다. 아래 "못 채운 것과 이유" |

## 못 채운 것과 이유

- **19 PDF 파일 내보내기.** tytower는 인쇄 화면(`/cms/portfolio-pdf/print`)을 열고 `window.print()`로 브라우저가 PDF로 저장하게 한다. 이 화면은 tytower 토큰이 있어야 데이터를 받는다. CLI가 PDF를 만들려면 헤드리스 브라우저를 띄우고 토큰을 넘겨야 하는데, tytower 쪽에 토큰을 URL로 받는 길이 없다(이 브랜치는 tytower를 고치지 않는다). 그래서 데이터(`portfolio_pdf` 섹션)까지만 CLI로 얻을 수 있다.
- **26 CMS 프록시.** 설계상 CLI는 cms-api를 직접 부르므로 대응하는 명령이 필요 없다.
- **11 이미지 보기**와 **24 로그인**은 부분이다. 이미지는 내려받지 않고 공개 주소만 보여 준다. ID/비밀번호는 저장하지 않고 필요할 때 묻는다.
- **49 카드 배치·정렬**은 명령이 없다(GUI 전용). 보드는 캔버스 위에 카드를 놓고, 선을 긋고, 화면을 옮기고 확대하는 공간 작업이다. 위치는 화면에서 보면서 정하는 것이라, 좌표를 숫자로 받는 명령은 결과를 확인할 수 없다. CLI는 보드를 **보고**(`boards show`), **이름을 바꾸고**(`boards rename`), **지우는** 것까지만 한다. 카드·선·화면 위치를 저장하는 쓰기는 화면이 한다.

**화면 상태 주의.** Overview(`/`)와 Brain(`/brain`)의 화면은 아직 placeholder다(`src/pages/overview.vue`, `placeholder.vue`). 그래서 이 두 그룹의 행은 화면이 아니라 HTTP API를 기준으로 채웠다. NetFile 화면(`/netfile`)과 CMS 화면은 실제로 있다.

## tytower에 없고 CLI에만 있는 것

`ci`, `ci --all`, `cms sync`/`sync-status`(Craft 동기화: tytower에는 없다), `cms posts --tag/--category/--search`(tytower 목록에는 검색과 필터가 없다), `cms portfolio put`/`section set`의 JSON 파일 입력(tytower는 폼으로 편집한다).

## 검증

- `cargo test`: 단위 테스트 109개 + 통합 테스트 38개(`tests/tower_cli.rs`, 흉내 낸 tytower에 실제 `tyange` 바이너리를 붙인다). 성공 출력, `--json`, `Authorization: Bearer` 헤더, 토큰이 출력에 없음, tytower가 꺼져 있을 때의 안내, `rm`이 `--yes` 없이 지우지 않음을 확인한다.
- 보드(행 45~48): 흉내 낸 보드 서버(`tests/support/boards.rs`, 계약의 검증과 revision 규칙을 따른다)로 목록(빈 것·여러 개), 보기, 이름 바꾸기(읽기 → 저장 순서, 저장 본문은 계약의 필드만), 그사이 revision이 바뀐 경우(409, 저장하지 않음), 없는 보드 이름 바꾸기(만들지 않음), `--yes` 없는 삭제(요청 0건), 404 안내, 잘못된 id·이름(요청 0건), 토큰 없음·틀린 토큰·tytower 꺼짐을 확인한다.
- 보드 저장 서버(`boards-contract.md`)는 이 저장소에 없다. 여기 검사는 계약을 흉내 낸 것이라 실제 tytower와 맞는지는 서버가 올라온 뒤에 한 번 더 봐야 한다.
- 실제 tytower(헤드리스, 포트 47890)에 대해 `tower status`, `overview`, `brain list`, `brain show`, `brain search`, `netfile status`를 실행했다. 끝나고 그 PID만 멈췄다.
