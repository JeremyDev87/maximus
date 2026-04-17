# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/main/LICENSE)

<p align="center">
  <a href="README.md">한국어</a> |
  <a href="README.en.md">English</a> |
  <a href="README.zh-CN.md">中文</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.ja.md">日本語</a>
</p>

为混乱的配置重建秩序。

Maximus 是一个 CLI，用来检查散落在项目各处的配置文件，整理冲突与重复项，并帮助团队维持有序的开发环境。

现代项目建立在大量配置之上，例如 `tsconfig`、`eslint`、`prettier`、`vite`、`jest`、`next.config`、`.env` 等。Maximus 可以在这些配置开始失控时重新建立秩序。

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
node ./bin/maximus.js audit
node ./bin/maximus.js fix --dry-run
```

## 适合这些团队

- 维护 monorepo / 多包仓库的团队
- 难以管理大量配置文件的团队
- 新成员经常在初始配置阶段卡住的团队

## 贡献

欢迎贡献。如果你想添加新的检查项、提升自动修复的安全性，或者减少误报，请先阅读 [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/main/CONTRIBUTING.md)。

## 安全

如果你怀疑发现了安全问题，请不要先公开提 issue。请按照 [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/main/SECURITY.md) 中的私密报告流程进行报告。

## 赞助

如果 Maximus 帮助你的团队减少了配置混乱，可以通过 [GitHub Sponsors](https://github.com/sponsors/JeremyDev87) 支持持续维护。

## 许可证

Maximus 基于 [MIT License](https://github.com/JeremyDev87/maximus/blob/main/LICENSE) 发布。
