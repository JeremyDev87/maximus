# npm wrapper runtime

## 목적

- 루트 `maximus` npm package를 thin launcher로 유지하면서 실제 실행은 Rust binary로 위임한다.
- 플랫폼별 binary package를 optional dependency로 두고, hard cutover 전까지는 packed install과 repo 개발 환경 모두에서 reference runtime fallback을 허용한다.

## 런타임 선택 순서

1. 현재 플랫폼과 아키텍처에 맞는 optional dependency package를 찾는다.
   - `maximus-darwin-arm64`
   - `maximus-darwin-x64`
   - `maximus-linux-arm64-gnu`
   - `maximus-linux-x64-gnu`
2. 설치된 platform package가 있으면 그 안의 `bin/maximus` Rust binary를 바로 실행한다.
3. repository 안에서 실행 중이고 `target/release/maximus` 또는 `target/debug/maximus`가 있으면 그 binary를 사용한다.
4. hard cutover 전까지 설치된 root package 안의 `src/cli.js` reference runtime으로 fallback한다.
5. 위 경로가 모두 없을 때만 wrapper가 실패한다.

## unsupported 정책

- 지원 플랫폼:
  - `darwin-arm64`
  - `darwin-x64`
  - `linux-arm64-gnu`
  - `linux-x64-gnu`
- hard cutover 전 미지원 플랫폼:
  - Windows
  - Linux musl
  - 기타 미지원 CPU 조합
- 미지원 플랫폼에서도 `src/cli.js` reference runtime이 남아 있는 동안은 JS fallback으로 계속 동작한다.
- reference runtime이 제거된 뒤에는 wrapper가 명확한 unsupported 오류 메시지를 출력해야 한다.

## package layout

- 루트 package:
  - `bin/maximus.js`
  - `src/**`
  - platform package를 가리키는 `optionalDependencies`
- platform package:
  - `package.json`
  - `bin/maximus`
- repository에 체크인된 `bin/maximus`는 placeholder다.
  - 실제 release pipeline은 이 파일을 플랫폼별 Rust executable로 교체해 publish한다.
  - local smoke에서는 helper script가 임시 platform package 복사본에 로컬 Rust binary를 주입한 뒤 pack/install 한다.

## local smoke

1. 루트 package tarball 생성:
   - `npm pack --json > /tmp/maximus-npm-pack.json`
2. packed wrapper smoke:
   - `node ./scripts/run-packed-wrapper-smoke.mjs /tmp/maximus-npm-pack.json ./test/fixtures/clean-project`

helper script는 다음을 보장한다.

- `npm pack --json`의 실제 `filename` 값을 읽는다.
- 현재 플랫폼 package 임시 복사본에 로컬 Rust binary를 주입한 뒤 pack/install 한다.
- optional runtime 검증 시에는 설치된 root package의 `src/` fallback을 제거해서 native binary 경로가 실제로 선택되도록 강제한다.
- 설치된 root wrapper 기준으로 `audit`, `doctor`, `fix --dry-run` smoke를 실행한다.
- `--omit=optional` 설치에서도 동일 명령이 `src/cli.js` fallback으로 성공하는지 검증한다.
