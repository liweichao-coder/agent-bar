# Codex Handoff Prompt

Paste the text below into a new Codex task opened at the cloned repository.

```text
你现在接手 Agent Bar 项目。请把当前 Git 仓库视为唯一可靠的项目上下文，不要假设你能访问旧电脑上的 Codex 对话。

先不要立即大改代码。请按顺序阅读：
1. AGENTS.md
2. docs/CURRENT_STATE.md
3. README.md
4. docs/product-requirements.md
5. docs/architecture-sketch.md
6. docs/iterations/0014-persistent-calendar-connections.md
7. docs/iterations/0015-week-calendar-schedule.md
8. docs/iterations/0016-rust-calendar-scheduler.md

然后执行以下工作：
1. 检查 git status、当前分支、最近提交和项目文件结构。
2. 安装缺失依赖，并运行 npm test、npm run build、cargo fmt --check、cargo test、cargo clippy -D warnings。
3. 如果环境允许，再运行 Tauri debug no-bundle 构建。
4. 用简洁中文告诉我：项目目前实现了什么、验证是否通过、新电脑环境有什么差异、下一步准备改哪些文件。
5. 在我确认环境正常后，继续下一迭代：为 Agent Bar 托管的 Codex 任务实现真实暂停与恢复。

下一迭代的边界：
- 为托管运行区分 running、pause-requested、paused、stop-requested、completed、failed。
- 暂停通过 turn/interrupt 发起；在中断前记录暂停意图。
- 恢复在同一 thread 上发起新的 turn/start，并使用清楚的继续任务指令。
- 终止也必须先记录 stop-requested，修复中断完成事件可能错误归类的竞态。
- 审批等待不能被误判为暂停。
- 界面只展示 Provider 真正支持的能力；优先级和跨 Agent 移交尚未实现时保持不可用。
- 为状态转换和 App Server 请求顺序补充聚焦测试。
- 完成后更新 docs/CURRENT_STATE.md，并新增下一编号的 docs/iterations 记录。

项目原则：
- local-first、隐私最小化。
- 不持久化 Agent prompt、思维链、工具输入输出、私有日历地址或截图。
- 不把模拟按钮写成真实能力。
- 不改写或伪造 Git 历史。
- 不撤销不属于当前任务的用户修改。
- 修改后必须验证，并清楚报告未验证的边界。
```

After the environment check, keep using the same Codex task for the focused
iteration. Use subagents only for independent research or review work; the main
task should retain architecture and integration ownership.
