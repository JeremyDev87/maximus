# VS Code Extension Draft

이 문서는 Maximus VS Code extension draft의 현재 계약을 설명합니다.

## 목표

첫 번째 버전은 아래 범위로 제한합니다.

- output channel 중심 실행
- CLI wrapper 제공
- `audit`, `doctor`, `fix` 명령만 우선 지원
- diagnostics provider, marketplace publish automation, telemetry 확장은 후속 작업으로 분리

## 실행 방식

확장은 VS Code command palette에서 실행됩니다.

- `Maximus: Run Audit`
- `Maximus: Run Doctor`
- `Maximus: Run Fix`

명령을 실행하면 output channel에 다음 정보를 남깁니다.

- 선택된 workspace 경로
- 실제로 실행된 CLI 명령
- stdout / stderr
- 종료 코드

## CLI wrapper 우선순위

1. workspace root에 `bin/maximus.js`가 있으면 `node`로 실행합니다.
2. 없으면 PATH 상의 `maximus` 실행 파일을 사용합니다.

이 순서는 로컬 저장소에서 직접 작업할 때와 설치된 CLI를 사용할 때를 모두 지원하기 위한 것입니다.

## 현재 상태

이 slice는 draft 수준의 소스와 문서만 추가합니다. 실제 VS Code 배포용 transpile / package 단계와 diagnostics 확장은 아직 포함하지 않습니다.
