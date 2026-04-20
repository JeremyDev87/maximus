# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)

<p align="center">
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.md">한국어</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.en.md">English</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.zh-CN.md">中文</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.es.md">Español</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.ja.md">日本語</a>
</p>

혼란스러운 설정에 질서를.

Maximus는 프로젝트 곳곳에 흩어진 설정 파일을 점검하고, 충돌과 중복을 정리하며 질서 있는 개발환경을 만드는 CLI입니다.

현대 프로젝트는 `tsconfig`, `eslint`, `prettier`, `vite`, `jest`, `next.config`, `.env` 등 수많은 설정 위에 서 있습니다. Maximus는 무너진 질서를 다시 세웁니다.

## Canonical Runtime

Maximus는 이제 Rust runtime을 canonical implementation으로 사용합니다.

- 루트 `maximus` npm package는 thin launcher이며, 실제 실행은 플랫폼별 prebuilt Rust binary로 위임합니다.
- 사용자 명령 표면은 그대로 유지합니다: `npx maximus audit`, `npx maximus doctor`, `npx maximus fix`
- 저장소의 `src/**/*.js`는 parity 검증과 reference 비교를 위한 frozen reference로 남아 있습니다. npm package에도 optional native runtime이 빠진 설치를 위한 compatibility fallback으로 포함되지만, canonical runtime으로는 취급하지 않습니다.
- `docs/plan/001`~`012`는 Rust v1 spec input이며, `docs/plan/013+`와 기존 JS backlog는 기본 구현 lane이 아닙니다.

전환 경계와 contributor 규칙은 [runtime transition 문서](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md)에서 확인할 수 있습니다.

## 주요 기능

- 설정 충돌 탐지
- 중복 config 소스 탐지
- 오래된 TypeScript 옵션 경고
- 잘못 연결된 path alias 검사
- ESLint / Prettier 충돌 분석
- 환경변수 누락 및 mismatch 검사
- 프로젝트 구조 리포트 생성

## 명령어

```bash
npx maximus audit
npx maximus doctor
npx maximus fix
```

### `audit`

현재 프로젝트의 설정 상태를 검사하고 핵심 리스크를 요약합니다.

### `doctor`

`audit`보다 더 설명적인 진단 모드입니다. 우선순위와 구조 개선 제안까지 함께 보여줍니다.

### `fix`

안전하게 자동 수정할 수 있는 항목만 적용합니다.

현재 MVP에서 지원하는 자동 수정:

- `.env` 기반 `.env.example` 생성
- `.env.example`에 누락된 키 추가

## 예시 출력

```text
Maximus audit
Target: /workspace/my-app

Status: attention needed
Findings: 1 error, 2 warnings, 1 info
Fixes available: 1

Findings
- [error] Path alias target does not exist
  file: packages/web/tsconfig.json
  detail: @ui/* points to src/missing/*
  hint: Update or remove the stale alias target.

- [warn] Missing .env.example contract
  file: .env
  detail: Runtime env files exist, but .env.example is missing.
  hint: Run `maximus fix` to create a blank contract file.
```

## GitHub Action

릴리즈 태그 이후에는 같은 npm wrapper 진입점을 GitHub Action에서도 그대로 사용합니다.

```yaml
- uses: JeremyDev87/maximus@<release-tag>
  with:
    command: audit
    path: .
```

기본 입력:

- `command`: `audit`, `doctor`, `fix`
- `path`: 검사할 프로젝트 경로, 기본값 `.`
- `<release-tag>`: publish된 릴리즈 태그로 바꿔 넣어야 합니다. 예: `v0.1.0`

## 로컬 개발

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

`node ./bin/maximus.js`는 repository 안에서 빌드된 Rust CLI(`target/debug/maximus`, `target/release/maximus`)를 우선 실행합니다. 로컬 바이너리가 아직 없으면 `cargo build -p maximus-cli`로 준비할 수 있습니다. `src/**/*.js`는 frozen reference로 남아 있고, npm 배포물에도 optional native runtime이 없는 설치를 위한 compatibility fallback으로 함께 포함됩니다.

## 이런 팀에 추천

- 모노레포 / 멀티패키지 운영 팀
- 설정 파일이 많아 관리가 어려운 팀
- 신규 입사자가 세팅에서 자주 막히는 팀

## 기여하기

새로운 점검기 추가, 자동 수정 안전성 개선, false positive 감소 같은 기여를 환영합니다. 다만 canonical runtime과 배포 표면은 이제 Rust 기준입니다. 기여 시작 전에는 [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md)와 [runtime transition 문서](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md)를 먼저 확인해 주세요.

## 보안

보안 이슈가 의심된다면 공개 이슈부터 열지 말고 [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md)의 비공개 신고 절차를 따라 주세요.

## 스폰서

Maximus가 팀의 설정 혼란을 줄이는 데 도움이 된다면 [GitHub Sponsors](https://github.com/sponsors/JeremyDev87)를 통해 유지보수를 후원할 수 있습니다.

## 라이선스

Maximus는 [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)로 배포됩니다.
