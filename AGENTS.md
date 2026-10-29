<!-- methodology:begin -->
Spec-driven work (`$kiro-*` skills, `.kiro/specs`, `.kiro/steering`): read `methodology/AGENTS.md` first. Other work does not need it.
<!-- methodology:end -->

## 릴리즈 원칙

사용자에게 보이는 변경을 `main`에 push하면 요청 없이 자동 릴리즈. 버전은 [SemVer](https://semver.org/lang/ko/) 준수.

- **대상**: 마지막 태그 이후 `feat`·`fix`·`perf`(출력·옵션·기본 동작 변경 포함)가 1건 이상
- **제외**: 문서·테스트·리팩터링·`chore`·`.kiro/`만 변경. 다음 릴리즈에 포함
- **버전 증가**(`0.x` 기준): 새 기능·옵션·기본 동작 변경·호환 깨짐은 MINOR, 결함 수정·성능 개선만 있으면 PATCH. 혼재 시 큰 쪽 적용
- **버전 위치**: `Cargo.toml`, `Cargo.lock`(`cargo build`가 갱신)만
- **절차**
  1. `git fetch --tags` 후 `git log vX.Y.Z..HEAD`로 대상 여부·증가 폭 판단
  2. `make test` 통과 확인
  3. `Cargo.toml`·`Cargo.lock`만 `dg X.Y.Z: 변경 요약`으로 커밋(`.kiro/` 등 타 파일 혼입 금지)
  4. 주석 태그 `git tag -a vX.Y.Z` 생성, 본문에 사용자 체감 변경 기재(이전 태그 본문 참고)
  5. `git push origin main vX.Y.Z`로 커밋·태그 동시 push
  6. 필요 시 `make install` 후 `dg --version` 확인
- **금지**: push한 태그 이동·삭제, force push
