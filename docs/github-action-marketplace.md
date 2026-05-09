# GitHub Action Marketplace Wrapper

이 문서는 `Maximus` GitHub Action을 Marketplace 친화적인 하위 경로 action으로 운영하는 기준을 정리합니다.

## 목적

- root [`action.yml`](../action.yml)은 현재 consumer contract의 source of truth로 유지합니다.
- `.github/actions/marketplace-wrapper/action.yml`은 Marketplace-friendly metadata와 문서화 책임만 가집니다.
- root action 입력 계약을 바꾸지 않고 같은 npm wrapper 실행 경로를 하위 action에서도 재사용합니다.

## 사용 경로

안정 태그를 발행한 뒤에는 root action 또는 Marketplace wrapper action 경로로 사용할 수 있습니다.

Root action:

```yaml
- uses: JeremyDev87/maximus@v1
  with:
    command: audit
    path: .
```

Marketplace wrapper action:

```yaml
- uses: JeremyDev87/maximus/.github/actions/marketplace-wrapper@v1
  with:
    command: audit
    path: .
```

기본 입력은 root action과 동일합니다.

- `command`: `audit`, `doctor`, `fix`
- `path`: 검사할 프로젝트 경로, 기본값 `.`
- `registry-url`: pre-release smoke나 사설 registry 검증이 필요할 때만 사용

## 버전 태그 전략

- stable consumer 예시는 moving major tag `v1`를 우선으로 안내합니다.
- 재현 가능한 pinning이 필요하면 `v1.0.0`처럼 immutable release tag를 사용합니다.
- `v1`은 `v1.0.0` 같은 immutable stable tag publish가 끝난 뒤에만 같은 commit으로 이동합니다.
- `v1`은 prerelease tag로 이동하지 않습니다.
- `v1` 이동은 npm publication trigger가 아니며, `release.yml`은 `v1.0.0` 같은 package release tag만 받습니다.
- `v1` 이동 후에는 `action-smoke.yml`을 `--ref v1`로 실행해서 root action과 marketplace wrapper action을 둘 다 검증합니다.

## 구현 원칙

- wrapper action은 repository root를 계산한 뒤 root package를 `npm install` 합니다.
- native runtime 확인도 root `scripts/assert-installed-native-runtime.mjs`를 그대로 사용합니다.
- root action 입력 계약과 실행 순서를 임의로 바꾸지 않습니다.

## 유지보수 체크리스트

- root `action.yml` 입력이 바뀌면 wrapper action 입력도 같은 turn에 동기화합니다.
- release smoke는 root action contract, marketplace wrapper contract, published tag 기준으로 검증합니다.
- `v1` tag 이동은 immutable stable tag publish와 smoke가 끝난 뒤 별도 확인을 받고 수행합니다.
- README 예시 추가가 필요하면 `README.md` / `README.en.md`를 소유한 별도 lane에서 처리합니다.
