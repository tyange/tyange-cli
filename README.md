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
tyange ci [--watch]   # 현재 리포의 Forgejo Actions 최근 실행 상태
```

## 로드맵

| 단계 | 서브커맨드 | 상태 |
|---|---|---|
| 0 | CLI 스켈레톤 | ✅ |
| 1 | `tyange ci` | 🚧 |
| 2 | `tyange ci --watch` | 예정 |
| 3 | `tyange repos` — 여러 리포 dirty/unpushed 상태판 | 예정 |
| 4 | `tyange publish <file>` — 마크다운 → CMS 발행 | 예정 |
| 5 | `tyange new <name> --template <t>` — 새 리포 스캐폴딩 | 선택 |

자세한 단계별 가이드는 `GUIDE.md`.

## 개발

```bash
cargo test    # 뭘 채워야 하는지는 실패하는 테스트가 알려준다
cargo run -- ci --help
```
