# tyange-cli

tyange 개인 인프라(블로그, CMS, Forgejo, 24개 리포)를 다루는 잡다한 기능 모음 CLI.
`gh`, `cargo`, `aws-cli`처럼 서브커맨드가 계속 늘어나는 구조다.
"완성"이 목표가 아니라, 새 마찰이 생길 때마다 서브커맨드 하나씩 추가하는 게 목표다.

## 왜 이 프로젝트인가

이미 fzf/rg/fd/bat/zoxide/delta 같은 성숙한 CLI 도구를 다 쓰고 있어서,
범용 도구를 새로 만드는 건 의미가 없다. 이건 **아무도 대신 만들어줄 수 없는 것**만 담는다 —
git.tyange.com(Forgejo), tyange-cms-api, 24개 개인 리포처럼 내 인프라에만 있는 것들.

## 지금 있는 것

```bash
tyange ci [--watch]            # 현재 리포의 Forgejo Actions 상태. --watch는 HEAD 커밋의 실행이 끝날 때까지
tyange ci --all [--recent]     # 모든 저장소에서 지금 돌고 있는 CI (--recent: 저장소마다 마지막 실행)
tyange cms posts|get|new|update|publish|draft|delete|tags   # 글·태그 관리
tyange cms image upload <파일>  # 이미지 업로드
tyange cms portfolio get|put|delete|section get|set   # 포트폴리오(PDF 포트폴리오 데이터 포함)
tyange cms guests               # 하객 명단·시간대별 인원
tyange cms keys list|create|revoke   # API 키 관리 (ID/비밀번호 로그인 필요)
tyange cms sync | sync-status  # Craft 폴더 동기화 즉시 실행 / 상태
tyange tower status                # tytower(데스크톱 컨트롤 타워)가 떠 있고 토큰이 맞는지
tyange tower overview [--refresh] [--source hn]   # 개요(Work·피드·관심사 소식)
tyange tower brain list|show|search|file|sync      # 세컨드 브레인 노트
tyange tower brain boards create|list|show|rename|delete  # 보드(화이트보드). 삭제는 --yes 필요, 카드 배치는 화면에서
tyange tower netfile status|search|ls|download|jobs|cancel|rm   # NetFile 검색과 파일 받기 (rm은 --yes 필요)
```

모든 명령은 `--json`을 받는다. 설정(`TYANGE_CMS_ID`, `TYANGE_CMS_PASSWORD`, `TYANGE_FORGEJO_TOKEN`)은
환경변수나 `~/.config/tyange-cli/env`에 둔다. tytower는 이 PC에서 켜져 있으면 설정 없이 붙는다(토큰 파일을 읽는다).
tytower 기능 대응표는 `docs/tower-parity.md`, 자세한 건 `CLAUDE.md`.

## 로드맵

| 단계 | 서브커맨드 | 상태 |
|---|---|---|
| 0 | CLI 스켈레톤 | ✅ |
| 1 | `tyange ci` | ✅ (Forgejo 실호출은 토큰 필요) |
| 2 | `tyange ci --watch` | ✅ (blocking 폴링) |
| 3 | `tyange repos` — 여러 리포 dirty/unpushed 상태판 | 예정 |
| 4 | `tyange cms new/update/publish` — 마크다운 → CMS 발행 | ✅ (프론트매터 대신 플래그) |
| 5 | `tyange new <name> --template <t>` — 새 리포 스캐폴딩 | 선택 |

자세한 단계별 가이드는 `GUIDE.md`.

## 개발

```bash
cargo test    # 뭘 채워야 하는지는 실패하는 테스트가 알려준다
cargo run -- ci --help
```
