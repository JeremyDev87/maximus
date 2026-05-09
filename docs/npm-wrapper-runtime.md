# npm wrapper runtime

## 목적

- 루트 `maximus` npm package를 thin launcher로 유지하면서 실제 실행은 Rust binary로 위임한다.
- 플랫폼별 binary package를 optional dependency로 두고, hard cutover 전까지는 packed install과 repo 개발 환경 모두에서 제한적 reference runtime fallback을 허용한다.
- placeholder platform package가 잘못 publish되더라도 hard cutover 전에는 wrapper가 이를 무시하고 JS reference runtime으로 fallback한다.
- v1.0.0의 정식 native runtime 표면은 macOS와 Linux glibc 4개 package로 고정한다.

## 런타임 선택 순서

1. 현재 플랫폼과 아키텍처에 맞는 optional dependency package를 찾는다.
   - `maximus-darwin-arm64`
   - `maximus-darwin-x64`
   - `maximus-linux-arm64-gnu`
   - `maximus-linux-x64-gnu`
2. repository 안에서 실행 중이고 `target/debug/maximus`가 있으면 그 binary를 사용한다.
3. `target/debug/maximus`가 없고 `target/release/maximus`가 있으면 그 binary를 사용한다.
4. repository local binary가 없고 설치된 platform package가 있으면 그 안의 `bin/maximus` Rust binary를 실행한다.
   - 단, placeholder marker(`MAXIMUS_RUST_BINARY_PLACEHOLDER`)가 있으면 실행하지 않고 다음 후보로 넘어간다.
5. hard cutover 전까지 설치된 root package 안의 `src/cli.js` reference runtime으로 제한적으로 fallback한다.
6. 위 경로가 모두 없을 때만 wrapper가 실패한다.

## unsupported 정책

- v1.0.0 native runtime 지원 플랫폼:
  - `darwin-arm64`
  - `darwin-x64`
  - `linux-arm64-gnu`
  - `linux-x64-gnu`
- v1.0.0 prebuilt native runtime 미지원 플랫폼:
  - Windows
  - Linux musl
  - 기타 미지원 CPU 조합
- 미지원 플랫폼에서도 `src/cli.js` reference runtime이 남아 있는 동안은 limited compatibility fallback으로만 동작한다.
- JS fallback 허용 범위는 config file이 없고 Rust-only flag가 없는 legacy-compatible `audit`, `doctor`, `fix --dry-run` 흐름이다.
- `maximus.config.json`, `.maximusrc.json`, `--only`, `--skip`, `--fail-on`, `--diff`, `--fix-id`, `--fix-prefix`, `--format`, `--output`, `fix` without `--dry-run`에는 native Rust runtime이 필요하다.
- Windows와 Linux musl은 v1.0.0에서 정식 native 지원 플랫폼으로 표시하지 않는다. JS fallback 제거는 별도 hard cutover 작업으로 다룬다.
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
  - wrapper는 placeholder marker가 남아 있으면 그 package를 실행 가능한 runtime으로 취급하지 않는다.

## local smoke

1. 루트 package tarball 생성:
   - `env npm_config_cache=/tmp/maximus-release-pack/.npm-cache npm pack --json --pack-destination /tmp/maximus-release-pack > /tmp/maximus-release-pack/pack.json`
2. packed wrapper smoke:
   - `node ./scripts/run-packed-wrapper-smoke.mjs /tmp/maximus-release-pack/pack.json ./test/fixtures/clean-project`

helper script는 다음을 보장한다.

- `npm pack --json`의 실제 `filename` 값을 읽는다.
- 현재 플랫폼 package 임시 복사본에 로컬 Rust binary를 주입한 뒤 pack/install 한다.
- optional runtime 검증 시에는 설치된 root package의 `src/` fallback을 제거해서 native binary 경로가 실제로 선택되도록 강제한다.
- 설치된 root wrapper 기준으로 `audit`, `doctor`, `fix --dry-run` smoke를 실행한다.
- `--omit=optional` 설치에서도 동일 명령이 `src/cli.js` fallback으로 성공하는지 검증한다.
