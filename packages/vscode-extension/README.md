# Maximus VS Code Extension

이 패키지는 Maximus CLI를 VS Code 안에서 실행하는 draft 확장입니다.

첫 버전의 기준은 다음과 같습니다.

- diagnostics provider 대신 output channel 중심으로 동작합니다.
- `audit`, `doctor`, `fix` 같은 CLI 명령을 그대로 감쌉니다.
- 로컬 저장소에서 `bin/maximus.js`가 있으면 그 경로를 우선 사용하고, 없으면 `maximus` 실행 파일을 찾습니다.

## 명령

- `Maximus: Run Audit`
- `Maximus: Run Doctor`
- `Maximus: Run Fix`

## 검증

패키지 루트에서 다음 검증을 돌릴 수 있습니다.

```bash
npm test
npm --prefix packages/vscode-extension test
```

이 draft는 아직 transpile / bundle 단계를 포함하지 않기 때문에, 실제 VS Code 배포 파이프라인은 후속 slice에서 마무리합니다.
