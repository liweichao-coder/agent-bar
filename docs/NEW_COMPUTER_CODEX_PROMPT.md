# New Computer Codex Bootstrap Prompt

下面整段文字可以直接发送给新电脑上的 Codex。它不假设项目已经克隆，
会从环境检查、安装建议、克隆仓库一直执行到项目验证和上下文恢复。

```text
我正在一台新的 Windows 电脑上继续开发 Agent Bar。请帮助我从零恢复开发环境和项目上下文。

项目仓库：
https://github.com/liweichao-coder/agent-bar

工作方式：
- 你可以主动运行只读检查、创建项目目录、克隆仓库、安装项目依赖和运行测试。
- 安装系统级软件、修改系统配置或需要管理员权限前，先告诉我将执行什么并等待我确认。
- 不要假设你能访问旧电脑的 Codex 对话、Memory、数据库或登录状态。
- GitHub 仓库是代码和项目上下文的唯一可靠来源。
- 不要改写 Git 历史，不要修改提交日期，不要提交本机数据或凭据。
- 在环境和基线验证完成前，不要开始新功能开发。

请按下面阶段执行，并在每个阶段遇到问题时先自行诊断。

第一阶段：检查 Windows 开发环境

1. 确认当前系统、PowerShell、CPU 架构和可用磁盘。
2. 检查以下命令是否可用，并记录版本：
   - git --version
   - node --version
   - npm --version
   - rustup --version
   - rustc --version
   - cargo --version
   - codex --version
3. 检查 Windows 是否具备 Tauri 2 所需环境：
   - Microsoft Visual Studio 2022 Build Tools
   - “Desktop development with C++”工作负载
   - Windows SDK
   - Microsoft Edge WebView2 Runtime
4. 如果缺少环境，先列出缺失项和安装方式，等待我确认后再安装：
   - Git 使用 Git for Windows；
   - Node.js 使用当前 LTS 版本并包含 npm；
   - Rust 使用官方 rustup 和 stable MSVC toolchain；
   - Visual Studio Build Tools 必须包含 Desktop development with C++；
   - WebView2 已存在时不要重复安装。
5. 优先使用官方安装源。若准备使用 winget，先用 winget search 核对当前
   package ID，再把准确安装命令发给我确认，不要凭记忆静默安装。
6. 参考：
   - Tauri Windows prerequisites: https://v2.tauri.app/start/prerequisites/
   - Rust installer: https://www.rust-lang.org/tools/install
   - Node.js LTS: https://nodejs.org/en/download
7. 安装完成后重新检查版本和 PATH。若新 PATH 尚未进入当前终端，明确告诉我需要重启 Codex 或终端。

第二阶段：选择目录并克隆仓库

1. 如果 D:\LwcCode 存在且可写，使用：
   D:\LwcCode\agent-bar
2. 否则使用：
   $HOME\source\agent-bar
3. 如果目标目录不存在，创建父目录并执行：
   git clone https://github.com/liweichao-coder/agent-bar.git <目标目录>
4. 如果目标目录已经是该仓库：
   - 先运行 git status；
   - 工作区干净时执行 git fetch origin，并仅进行正常的 fast-forward 更新；
   - 工作区有修改时不要覆盖、reset 或 checkout，先向我报告。
5. 进入仓库后检查：
   - git remote -v
   - git branch --show-current
   - git status --short
   - git log --oneline -5
6. 默认应位于 main 分支，并且本地 HEAD 应与 origin/main 一致。

第三阶段：安装项目依赖

1. 先阅读 package.json、package-lock.json、src-tauri/Cargo.toml 和
   src-tauri/tauri.conf.json，确认实际工具链。
2. 新克隆仓库优先执行 npm ci。
3. 使用 rustup 确认 stable MSVC toolchain 可用。
4. 不要安装仓库未使用的全局 npm 包。
5. 不要复制旧电脑的 node_modules、dist、src-tauri/target 或日志。

第四阶段：恢复项目上下文

请按顺序阅读：
1. AGENTS.md
2. docs/CURRENT_STATE.md
3. README.md
4. docs/product-requirements.md
5. docs/architecture-sketch.md
6. docs/research-notes.md
7. docs/iterations/0014-persistent-calendar-connections.md
8. docs/iterations/0015-week-calendar-schedule.md
9. docs/iterations/0016-rust-calendar-scheduler.md

其中 AGENTS.md 只约束 Agent Bar 仓库及其子目录，不要把它应用到其他项目。

第五阶段：验证项目基线

在仓库根目录依次运行：

npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri build -- --debug --no-bundle

注意：
- 如果 cargo 未进入当前 PATH，可以临时调用
  & "$env:USERPROFILE\.cargo\bin\cargo.exe"
- Rust 常规测试中有少量依赖 Windows Credential Manager 或已登录 Codex 的测试按设计被忽略，不要把“ignored”直接判断为失败。
- 不要为了让测试通过而删除测试、放宽隐私约束或提交本机凭据。
- 构建生成的 dist、target、日志、Tauri schemas、SQLite 文件不能进入 Git。

第六阶段：向我汇报

完成后用简洁中文汇报：
1. 项目克隆到哪个绝对路径；
2. Git、Node、npm、Rust、Cargo 和 Codex 的版本；
3. 当前分支、HEAD 提交和工作区是否干净；
4. Node 测试、前端构建、Rust 测试、fmt、Clippy 和 Tauri 构建的结果；
5. 哪些环境敏感测试被忽略；
6. 与旧仓库文档基线相比有哪些环境差异；
7. 是否已经具备继续开发的条件。

如果验证全部通过，先停下来等我确认，不要立刻修改功能。

我确认后，下一迭代是为 Agent Bar 托管的 Codex 任务实现真实暂停与恢复：
- 区分 running、pause-requested、paused、stop-requested、completed、failed；
- 暂停通过 turn/interrupt 发起，并在中断前记录暂停意图；
- 恢复在同一 thread 上发起新的 turn/start；
- 终止前记录 stop-requested，修复中断完成事件错误归类的竞态；
- 审批等待不能被误判为暂停；
- 只展示 Provider 真正支持的能力；
- 优先级和跨 Agent 移交未实现时保持不可用；
- 补充状态转换和 App Server 请求顺序测试；
- 完成后更新 docs/CURRENT_STATE.md，并新增下一编号的迭代记录。

本项目继续遵守：
- local-first 和隐私最小化；
- 不持久化 Agent prompt、思维链、工具输入输出、私有日历地址或截图；
- 不把模拟按钮描述成真实能力；
- 不撤销不属于当前任务的用户修改；
- 修改后必须运行验证，并清楚报告尚未验证的边界。
```

## Optional Local Data

代码开发不需要迁移旧电脑的应用数据库。只有确实需要保留演示数据时，
才单独复制：

```text
%APPDATA%\com.agentbar.desktop\agent-bar.sqlite3
```

不要把该文件上传 GitHub、附加到云端对话或放进仓库。日历私有地址保存
在 Windows Credential Manager 中，应在新电脑重新授权，而不是导出凭据。
