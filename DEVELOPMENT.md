# DEVELOPMENT.md

> 二次开发工作流（macOS, internal fork）。
> 配套阅读：`AGENTS.md`（规则）、`ARCHITECTURE.md`（地图）、`WARP.md`（命令 + 风格）。

## 0. 你应该用哪条入口

| 你想做什么 | 用什么 |
| ---------- | ------ |
| 启动 App，看效果 | `cargo run --bin warp-oss` |
| 跑一次环境体检 | `./init.sh` |
| 提交前最终保险 | `./script/presubmit` |
| 写代码 / 改代码 | Cursor 或 VS Code，配 `rust-analyzer` |
| 让 AI 帮你改 | Cursor / Claude Code，先让它们读 `AGENTS.md` |

## 1. 环境前置

### 已完成（这台机器）

- ✅ `rustup 1.29.0`，自动激活 `1.92.0`（受 `rust-toolchain.toml` 控制）
- ✅ `~/.cargo/bin` 已在 PATH（`init.sh` 也会强制 export）
- ✅ `cargo-binstall`, `diesel`, `cargo-bundle`, `cargo-about`,
  `cargo-nextest`, `wgslfmt` 已装
- ✅ `cargo check` 在 default-members 上**已经过**（耗时 2m12s）

### 仍未做（按需补）

| 任务 | 何时需要 | 怎么做 |
| ---- | -------- | ------ |
| `corepack enable` + Node 18+ | 当你要构建 `completions_v2` 或 `command-signatures-v2` 时 | `corepack enable && cd crates/command-signatures-v2/js && yarn install` |
| `script/macos/install_build_deps`（Metal Toolchain） | 第一次 `cargo run` 真正走完链接时；**已经在前面 bootstrap 中跑过了** | 报错时再补 |
| `brew install jq sentry-cli clang-format pkgconf llvm` | 跑 `script/presubmit` 全套时 | 按需 `brew install <name>` |
| `gcloud auth login` | 你需要 Warp 内部 channel config 时 | **二开内部版基本不用，跳过** |
| `docker` / `powershell` / `create-dmg` / `multitime` | 打包发布、PowerShell 相关时 | 内部版不发布就不装 |

> **结论**：`./script/bootstrap` 不要整套跑，按报错增量补依赖即可。

## 2. 加速构建（最重要）

Warp 是 60+ crate 的 workspace，**冷编译 5–10 分钟，链接 30s+**。
下面这些做完，**热编译能压到 10–30 秒**。

### 2.1 一次性配置：链接器 + 共享 target

把下面贴进 `~/.cargo/config.toml`（**用户级**，不是仓库内的，否则会污
染上游仓库）：

```toml
# ~/.cargo/config.toml
[target.aarch64-apple-darwin]
# Use lld for ~3-5x faster linking. Install: `brew install llvm`
# rustflags = ["-C", "link-arg=-fuse-ld=/opt/homebrew/opt/llvm/bin/ld64.lld"]
# Or use the system linker but skip dead-code stripping in dev:
rustflags = ["-C", "link-arg=-Wl,-no_warn_duplicate_libraries"]

[build]
# Share one target/ across all your Rust projects (huge SSD save).
# target-dir = "/Users/troye/.cargo-target"
# Optional: use sccache to cache compilation (`cargo install sccache`).
# rustc-wrapper = "sccache"
```

> 仓库内 `.cargo/config.toml` 已经设了 `MACOSX_DEPLOYMENT_TARGET` 等
> 关键参数，**不要改它**。把上面这些加速选项放在用户级配置里。

### 2.2 Cargo profile：dev 编译期降优化

把这段加进仓库根 `Cargo.toml` 末尾（已是工作区根，对所有 crate 生
效；这是上游 Warp 团队也常用的写法）：

```toml
[profile.dev]
# Warp 默认 dev 已经够保守，但如果你想极致快，可以试：
# debug = "line-tables-only"      # 体积/速度折中
# split-debuginfo = "unpacked"    # macOS 上链接更快
```

> **先不要动**，等你真感到链接慢再补。Warp 默认 dev 已经合理。

### 2.3 实战命令

```bash
# 最快 = 只检查 default-members（不带 --workspace），不链接
cargo check                                  # 你已经验证过 ~2m12s

# 单 crate check（最快迭代）
cargo check -p warp_core
cargo check -p warp_terminal

# 单 crate test（避免编全量）
cargo test -p warp_core
cargo nextest run -p warp_core

# 跑一次完整客户端
cargo run --bin warp-oss

# 想跑全量测试（慢，10–20 分钟，需要 Node）
cargo nextest run --no-fail-fast --workspace --exclude command-signatures-v2
```

### 2.4 IDE 配置（rust-analyzer）

Cursor / VS Code 里建议（`.vscode/settings.json` 已有，确认这几项）：

```jsonc
{
  "rust-analyzer.cargo.allFeatures": false,         // 关键：不要 all-features，否则 RA 会卡
  "rust-analyzer.cargo.features": [],               // 留空，RA 自己用 default
  "rust-analyzer.check.command": "check",           // 不用 clippy 做实时检查（太慢）
  "rust-analyzer.check.workspace": false,           // 关键：只 check 当前 crate
  "rust-analyzer.cargo.buildScripts.enable": true,
  "rust-analyzer.procMacro.enable": true,
  "rust-analyzer.linkedProjects": ["./Cargo.toml"]
}
```

## 3. 二开模块化策略

**不要直接改 `app/` 里现有文件再 commit 上去**——这样上游一动就冲突。
按下面分层：

### 3.1 新功能 = 新 crate / 新模块 + Cargo feature flag

1. 在 `app/src/` 或 `crates/` 下**新增**模块/crate（不动旧的）。
2. 在 `app/Cargo.toml [features]` 加一个 feature，gate 你的新代码。
3. 上游同步时，绝大多数 merge 冲突会落在 feature 列表的添加位置（小
   而易解）。
4. 内部默认开启你的 feature；OSS 上游版关掉它仍能编译。

模板 / 套路 → `.claude/skills/add-feature-flag/SKILL.md`

### 3.2 改现有行为 = patch crate 或 trait 注入

如果非要改现有行为：

- 优先**新增** trait + 默认实现，把旧逻辑作为默认，新逻辑作为可选实现。
- 再不行，**最小化 hunk**：只改最少的几行，commit 信息写清楚为什么。
- 给 patch 加 `// FORK: <reason>` 标记，方便 grep 出所有魔改点：
  ```bash
  rg "FORK:" --type rust
  ```

### 3.3 上游同步策略

```bash
# 内部 master ← 上游 master（用 rebase 而不是 merge）
git remote add upstream https://github.com/warpdotdev/warp.git
git fetch upstream
git rebase upstream/master
# 冲突出现时，参考 .claude/skills/resolve-merge-conflicts/SKILL.md
```

每次同步前先**记录 baseline**：

```bash
git log --oneline upstream/master ^master | head -20  # 上游领先了哪些
```

## 4. AI Agent 协作流（你的核心痛点之一）

这个仓库**已经为 Agent 协作做了大量准备**，把这些用起来就能大幅省时
间。

### 4.1 已有的资产

| 资产 | 在哪 | 干嘛用 |
| ---- | ---- | ------ |
| `WARP.md` | 根目录 | Warp/Cursor/Claude Code 启动时自动读，给 Agent 项目上下文 |
| `AGENTS.md` | 根目录（这次新建） | Agent 操作规则 + Definition of Done |
| `.claude/skills/*/SKILL.md` | `.claude/skills/` | 18 个领域技能：feature-flag、telemetry、create-pr、resolve-merge-conflicts、warp-ui-guidelines、warp-integration-test、rust-unit-tests、fix-errors、diagnose-ci-failures... |
| `specs/APP-*/` | `specs/` | 129+ 已有 feature 的 PRODUCT/TECH 规范——给 Agent 看比给它看代码更高效 |
| `.cursor/` rules | `.cursor/` | Cursor 项目级规则（如有） |
| `.warp/launch_configurations` | `.warp/` | Warp 自家的 launch 配置 |

### 4.2 让 Agent 高效改代码的工作流

**单次会话推荐流程**：

```
1. 你 → Agent: "改一下 Agent Mode 的 XXX 行为"
2. Agent: 读 AGENTS.md → 读 progress.md → 读 feature-list.json
3. Agent: rg "XXX" specs/  → 找到对应 spec
4. Agent: rg "XXX" app/src/ai/  → 找到代码
5. Agent: 调用 .claude/skills/add-feature-flag/SKILL.md（如果是新行为）
6. Agent: 改代码 + 写测试
7. Agent: cargo check -p <crate>  → cargo nextest run -p <crate>
8. Agent: 更新 feature-list.json + progress.md
9. Agent: commit（不 push）
```

**让 Agent 用增量构建（关键省时）**：在 prompt 里明确说

> 改完每个 crate 之后，先用 `cargo check -p <crate>` 验证，不要直接
> 跑 `cargo build --workspace`。

### 4.3 给 Agent 的"启动咒语"

下次开新会话第一句直接给：

```
读 AGENTS.md 和 ARCHITECTURE.md 和 progress.md。我要做的事情是：[你的需求]。
开始前先告诉我你打算改哪些文件、怎么验证。
```

## 5. 调试 / 排错

### 5.1 编译报错

| 报错关键字 | 可能原因 | 怎么修 |
| ---------- | -------- | ------ |
| `Failed to build command signatures JS` | 没装 Node + corepack | 不带 `--workspace` 编；或装 Node + `corepack enable` |
| `linker '...' not found` | 链接器配错 | 把 `~/.cargo/config.toml` 里的 lld 路径注释掉 |
| `Cargo.lock outdated` | rustup 没切对工具链 | `rustup show`，确认 active = 1.92.0 |
| `invalid mach-O signature` | macOS Gatekeeper 拦截没签名的 dylib | `xattr -cr target/debug/<bundle>` |
| `Metal toolchain` 相关 | 没下 Metal Toolchain | `xcodebuild -downloadComponent MetalToolchain` |

### 5.2 运行时调试

```bash
# 详细日志（Warp 用 tracing）
RUST_LOG=warp=debug,info cargo run --bin warp-oss

# 单条命令的 debug dump（Warp 自带）
# 在 app 里按快捷键 / 命令面板找 "Debug Dump"

# Sentry 关掉（oss 默认就没开）
# 走 oss 入口即可
```

### 5.3 做 UI 改动时

读 `.claude/skills/warp-ui-guidelines/SKILL.md`——里面是 UI 改动的所
有 do/don't（mouse handle、context 命名、format-args 等）。

### 5.4 写测试

- **单元测试** → `.claude/skills/rust-unit-tests/SKILL.md`
- **集成测试** → `.claude/skills/warp-integration-test/SKILL.md`
- **修编译/lint 错误** → `.claude/skills/fix-errors/SKILL.md`

## 6. 常见任务速查

| 我要... | 命令 / 路径 |
| ------- | ----------- |
| 启动 App | `cargo run --bin warp-oss` |
| 改完只验当前 crate | `cargo check -p <crate> && cargo nextest run -p <crate>` |
| 提 PR 前全检查 | `./script/presubmit` |
| 加 feature flag | `.claude/skills/add-feature-flag/` |
| 加埋点 | `.claude/skills/add-telemetry/` |
| 看上游领先了哪些 | `git fetch upstream && git log --oneline master..upstream/master` |
| 找一个 feature 在哪改 | `rg "feature_name" specs/ && rg "feature_name" app/src/` |
| 看某个 crate 怎么编译 | `cargo build -p <crate> --timings` 后开 `target/cargo-timings/cargo-timing.html` |
| 清掉脏的 target | `cargo clean -p <crate>` 而不是 `cargo clean`（避免重新编译全部依赖） |

## 7. 一些"反直觉"的注意点

1. **不要全量 `cargo build --workspace`**：会把
   `command-signatures-v2`（要 Node）和 `serve-wasm`（不必要）都拖进
   来。日常用 `cargo check`/`cargo run` 即可。
2. **不要全量 `cargo nextest run --workspace`** 而不带 `--exclude command-signatures-v2`：会跑半天然后挂在 JS 构建上。
3. **`MouseStateHandle::default()` 是 silent bug**：UI 鼠标失效但不报
   错。读 `WARP.md` 的 WarpUI 注意事项。
4. **`unused parameter` 不要加下划线，要直接删**：项目风格如此（见
   `WARP.md`）。
5. **格式化字符串用内联**：`format!("{x}")` 而不是 `format!("{}", x)`，
   触发 `uninlined_format_args` clippy。
6. **`Cargo.lock` 提交到仓库**：是项目策略；不要 `cargo update` 后无
   理由提交。

## 8. 下一步建议

按这个顺序走，一周内你能把 Warp 摸清：

1. **Day 1**：`cargo run --bin warp-oss` 跑起来 → App 窗口能开。
2. **Day 1–2**：选一个**最小可见改动**练手（比如改窗口标题字符串、
   改默认主题颜色），走完"改→编→运行→看到效果"完整闭环。
3. **Day 2–3**：读 `ARCHITECTURE.md` + 浏览 `app/src/` 顶层目录，对
   每个领域有印象。
4. **Day 3–5**：选一个你真正关心的领域（AI / UI / Terminal），读对
   应 `specs/APP-*` 里 2–3 个 PRODUCT.md，理解它们怎么写需求。
5. **Day 5+**：开始你真正的二开 feature，按 §3 的流程隔离改动。
