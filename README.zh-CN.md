# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)

<p align="center">
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.md">한국어</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.en.md">English</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.zh-CN.md">中文</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.es.md">Español</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.ja.md">日本語</a>
</p>

为混乱的配置重建秩序。

Maximus 是一个 CLI，用来检查散落在项目各处的配置文件，整理冲突与重复项，并帮助团队维持有序的开发环境。

现代项目建立在大量配置之上，例如 `tsconfig`、`eslint`、`prettier`、`vite`、`jest`、`next.config`、`.env` 等。Maximus 可以在这些配置开始失控时重新建立秩序。

## Canonical Runtime

Maximus 现在使用 Rust runtime 作为 canonical implementation。

- 根 `maximus` npm package 是一个 thin launcher，实际执行委托给按平台分发的 prebuilt Rust binary。
- 面向用户的命令入口保持不变：`npx maximus audit`、`npx maximus doctor`、`npx maximus fix`
- `src/**/*.js` 仍保留在仓库中，作为用于 parity 和比较的 frozen reference code。它也会继续随 npm package 一起分发，作为缺少 optional native runtime package 时的 compatibility fallback，但不再被视为 canonical runtime。
- `docs/plan/001` 到 `012` 是 Rust v1 的 spec input，而 `docs/plan/013+` 和旧的 JS backlog 已不再是默认实现路径。

迁移边界、阶段划分和贡献规则可见 [runtime transition 文档](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md)。

## 功能

- 检测配置冲突
- 检测重复的配置来源
- 警告过时的 TypeScript 选项
- 检查错误的路径别名连接
- 分析 ESLint / Prettier 冲突
- 检查缺失或不匹配的环境变量
- 生成推荐的项目结构报告

## 命令

```bash
npx maximus audit
npx maximus doctor
npx maximus fix
```

### `audit`

检查项目当前的配置状态，并汇总风险最高的问题。

### `doctor`

比 `audit` 更具解释性的诊断模式，会额外给出优先级和结构改进建议。

### `fix`

只应用安全的自动修复。

当前 MVP 支持的自动修复：

- 根据实际 `.env` 文件生成 `.env.example`
- 将缺失的键追加到 `.env.example`

## 输出示例

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

## 本地开发

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

`node ./bin/maximus.js` 会优先运行仓库内构建好的 Rust CLI（`target/debug/maximus`、`target/release/maximus`）。如果你还没有本地 binary，可以通过 `cargo build -p maximus-cli` 构建。`src/**/*.js` 会继续作为 frozen reference code 保留，也会随 npm wrapper package 一起分发，作为缺少 optional native package 时的 compatibility fallback。

## 适合这些团队

- 维护 monorepo / 多包仓库的团队
- 难以管理大量配置文件的团队
- 新成员经常在初始配置阶段卡住的团队

## 贡献

欢迎贡献。如果你想添加新的检查项、提升自动修复的安全性，或者减少误报，请先阅读 [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md) 和 [runtime transition 文档](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md)，因为 canonical runtime 和分发表面现在都以 Rust 为先。

## 安全

如果你怀疑发现了安全问题，请不要先公开提 issue。请按照 [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md) 中的私密报告流程进行报告。

## 赞助

如果 Maximus 帮助你的团队减少了配置混乱，可以通过 [GitHub Sponsors](https://github.com/sponsors/JeremyDev87) 支持持续维护。

## 许可证

Maximus 基于 [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE) 发布。
