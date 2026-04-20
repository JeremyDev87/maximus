# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)

<p align="center">
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.md">한국어</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.en.md">English</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.zh-CN.md">中文</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.es.md">Español</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.ja.md">日本語</a>
</p>

混沌とした設定に秩序を。

Maximus は、プロジェクトのあちこちに散らばった設定ファイルを監査し、競合や重複を整理して、整った開発環境を作るための CLI です。

現代のプロジェクトは `tsconfig`、`eslint`、`prettier`、`vite`、`jest`、`next.config`、`.env` など数多くの設定の上に成り立っています。Maximus は、その構成が崩れ始めたときに秩序を取り戻します。

## Canonical Runtime

Maximus は現在、Rust runtime を canonical implementation として使用します。

- ルートの `maximus` npm package は thin launcher であり、実際の実行はプラットフォーム別の prebuilt Rust binary に委譲されます。
- ユーザー向けコマンドの表面は維持されます: `npx maximus audit`、`npx maximus doctor`、`npx maximus fix`
- `src/**/*.js` は parity 作業や比較のための frozen reference code としてリポジトリに残ります。optional native runtime package が入らないインストール向けの compatibility fallback として npm package にも同梱されますが、canonical runtime としては扱われません。
- `docs/plan/001` から `012` は Rust v1 の spec input であり、`docs/plan/013+` と以前の JS backlog は、もはや既定の実装レーンではありません。

移行境界、フェーズ構成、コントリビューター向けルールは [runtime transition ドキュメント](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) を参照してください。

## 主な機能

- 設定の競合を検出
- 重複した設定ソースを検出
- 古い TypeScript オプションを警告
- 壊れた path alias を検査
- ESLint / Prettier の競合を分析
- 不足または不一致の環境変数を検査
- 推奨プロジェクト構成レポートを生成

## コマンド

```bash
npx maximus audit
npx maximus doctor
npx maximus fix
```

### `audit`

プロジェクトの現在の設定状態を確認し、主要なリスクを要約します。

### `doctor`

`audit` よりも説明的な診断モードで、優先順位や構成改善の提案も表示します。

### `fix`

安全に適用できる自動修正のみを実行します。

現在の MVP で対応している自動修正:

- 実際の `.env` ファイルから `.env.example` を生成
- `.env.example` に不足しているキーを追加

## 出力例

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

## ローカル開発

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

`node ./bin/maximus.js` は、リポジトリ内でビルドされた Rust CLI（`target/debug/maximus`、`target/release/maximus`）を優先します。まだローカル binary がない場合は、`cargo build -p maximus-cli` で用意できます。`src/**/*.js` は frozen reference code として残り、optional native package がないインストール向けの compatibility fallback として npm wrapper package にも同梱されます。

## こんなチームにおすすめ

- モノレポ / マルチパッケージを運用するチーム
- 多数の設定ファイルの管理が難しいチーム
- 新しく参加したメンバーがセットアップで詰まりやすいチーム

## コントリビュート

新しいチェックの追加、自動修正の安全性向上、false positive の削減などの貢献を歓迎します。まずは [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md) と [runtime transition ドキュメント](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) を確認してください。canonical runtime と配布表面は現在 Rust-first です。

## セキュリティ

セキュリティ上の問題を見つけた可能性がある場合は、まず公開 issue を作成しないでください。非公開の報告手順は [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md) を参照してください。

## スポンサー

Maximus がチームの設定混乱を減らすのに役立っているなら、[GitHub Sponsors](https://github.com/sponsors/JeremyDev87) から継続的なメンテナンスを支援できます。

## ライセンス

Maximus は [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE) のもとで公開されています。
