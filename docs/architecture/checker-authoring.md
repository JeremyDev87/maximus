# Checker Authoring

이 문서는 Maximus 저장소에서 checker를 작성하고 연결하는 현재 구조를 정리한다.

Maximus의 canonical runtime은 Rust다. `src/**/*.js`는 frozen reference로 남아 있으며, 새 checker 구현과 배포 동작의 기준은 Rust 쪽에 있다.

## 현재 구조

체커 구현은 Rust workspace 안에 있다.

- 체크 실행과 등록은 [`crates/maximus-checks/src/lib.rs`](../../crates/maximus-checks/src/lib.rs)와 [`crates/maximus-checks/src/registry.rs`](../../crates/maximus-checks/src/registry.rs)에 모여 있다.
- 개별 checker 로직은 [`crates/maximus-checks/src/*.rs`](../../crates/maximus-checks/src) 아래에 있다. 현재는 `config_duplicates.rs`, `env.rs`, `eslint_prettier.rs`, `lockfiles.rs`, `package_entrypoints.rs`, `structure.rs`, `tsconfig.rs`가 이 경계 안에 있다.
- 공통 데이터 모델과 프로젝트 스냅샷은 [`crates/maximus-core/src/models.rs`](../../crates/maximus-core/src/models.rs)와 [`crates/maximus-core/src/lib.rs`](../../crates/maximus-core/src/lib.rs)에 있다.
- CLI는 [`crates/maximus-cli/src/main.rs`](../../crates/maximus-cli/src/main.rs)에서 `registered_check_ids()`를 검증하고, `audit_project_with_config_root()`를 통해 checker 결과를 실행한다.
- JS reference는 [`src/checks/*.js`](../../src/checks)와 [`src/core/*.js`](../../src/core)에 남아 있지만, 현재 구조의 canonical implementation 표면은 아니다.

## Checker 흐름

현재 registry에 들어가는 checker는 다음 순서로 동작한다.

1. `maximus-core`가 프로젝트를 스캔해서 [`ProjectSnapshot`](../../crates/maximus-core/src/models.rs)를 만든다.
2. `maximus-checks`의 registry가 등록된 check id를 순회한다.
3. 각 checker는 `ProjectSnapshot`과 `MaximusConfig`를 받아 [`CheckOutcome`](../../crates/maximus-checks/src/check_outcome.rs) 을 반환한다.
4. registry가 모든 outcome을 합쳐 하나의 audit 결과로 만든다.
5. CLI가 `audit`, `doctor`, `fix` 출력과 종료 코드를 정한다.

현재 registry에 등록된 checker id는 다음과 같다.

- `duplicates`
- `env`
- `eslint-prettier`
- `tsconfig`
- `lockfiles`
- `package-entrypoints`

## 새 checker 작성

현재 저장소 구조에서 새 checker를 추가할 때는 Rust 쪽에 다음 항목을 맞춰 넣는다.

1. `crates/maximus-checks/src/` 아래에 새 모듈을 만든다.
2. `crates/maximus-checks/src/lib.rs`에서 모듈을 노출한다.
3. `crates/maximus-checks/src/registry.rs`의 `REGISTERED_CHECKS`와 `registered_check_ids()`에 id를 추가한다.
4. 필요한 데이터는 `maximus-core`의 모델과 유틸을 재사용한다.
5. `crates/maximus-checks/tests/`에 회귀 테스트를 둔다.

`CheckOutcome`는 현재 세 필드로 구성된다.

- `findings`
- `fixes`
- `planned_fixes`

`fix` 명령은 초기 분석에서 `planned_fixes`와 `fixes` 수가 같을 때만 실행 가능한 수정으로 간주한다. 새 checker가 수정 가능 항목을 만든다면 이 둘의 관계를 함께 맞춰야 한다.

## 테스트 위치

현재 checker 관련 테스트는 주로 crate별 통합 테스트에 있다.

- [`crates/maximus-checks/tests/basic_checks.rs`](../../crates/maximus-checks/tests/basic_checks.rs)
- [`crates/maximus-checks/tests/structure_checks.rs`](../../crates/maximus-checks/tests/structure_checks.rs)
- [`crates/maximus-checks/tests/tsconfig_checks.rs`](../../crates/maximus-checks/tests/tsconfig_checks.rs)
- [`crates/maximus-checks/tests/lockfiles_checks.rs`](../../crates/maximus-checks/tests/lockfiles_checks.rs)
- [`crates/maximus-checks/tests/env_checks.rs`](../../crates/maximus-checks/tests/env_checks.rs)
- [`crates/maximus-checks/tests/package_entrypoints_checks.rs`](../../crates/maximus-checks/tests/package_entrypoints_checks.rs)

테스트는 checker가 어떤 finding id, severity, detail, hint, file 경로를 내는지 고정하는 역할을 한다. 현재 저장소에서는 경로 렌더링과 진단 문구가 JS reference와 일치하는지도 테스트에서 확인한다.

checker 변경이 CLI의 텍스트 출력, JSON shape, exit semantics, fix preview까지 흔들면 그 계약도 함께 고정해야 한다. 이런 변경은 `test/reference-parity.test.js`, `test/golden-rust/*`, `crates/maximus-cli/tests/mvp_parity.rs`, 관련 CLI 통합 테스트 중 맞는 지점을 같은 PR에서 같이 갱신하는 것이 기본이다. 변경이 launcher resolution이나 wrapper-visible behavior까지 닿으면 `test/wrapper-runtime.test.js`를 추가로 보고, packed-install 또는 fallback 경계가 바뀌면 `test/packed-wrapper-fallback.test.js`도 같이 검증한다.

## 경계

- Rust runtime이 canonical implementation이다.
- `src/**/*.js`는 parity와 reference 확인을 위한 고정된 비교 대상이다.
- 새 checker 작성은 Rust crate 쪽에 들어가야 한다.
- README, CONTRIBUTING, runtime transition 문서는 이 경계를 같은 표현으로 유지해야 한다.
