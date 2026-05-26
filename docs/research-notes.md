# Similar Project Notes

调研日期：2026-05-26

## ActivityWatch

定位：开源、跨平台、本地优先的自动时间追踪工具。

值得借鉴：

- 采用 server + watcher + web UI 的松耦合结构。
- watcher 用 heartbeat 上报当前窗口、AFK、浏览器标签等活动。
- 数据按 bucket 组织，事件带 timestamp/duration/data，适合做日历和时间线聚合。
- UI 有 dashboard 和 timeline，可作为“每日活动可视化”的参考。

不直接照搬：

- 它偏“事后统计”，Agent Bar 更需要“实时计划 + 实时 agent 状态 + 可介入控制”。
- 它的 UI 是完整 dashboard，不是桌面常驻 bar。

## OpenHands

定位：面向软件开发 agent 的平台，包含 SDK、CLI、本地 GUI 和云端 GUI。

值得借鉴：

- 把 agent engine、CLI、GUI 拆开，说明核心 agent 状态和 UI 不应该强耦合。
- 本地 GUI 提供 REST API + React 单页应用，这对 Agent Bar 的“本地 daemon + desktop UI”很有启发。
- 适合作为“agent 在做什么”的行为可视化参考：任务、终端、浏览器、代码修改、日志流。

不直接照搬：

- OpenHands 聚焦 coding agent，Agent Bar 应该更泛化：日程、学习、资料整理、科研工作流、桌面自动化都要能接入。

## Langfuse

定位：开源 LLM engineering/observability 平台，核心是 trace、prompt、eval、dataset、metrics。

值得借鉴：

- 对 agent action、retrieval、embedding、LLM call 做 trace，适合解释“为什么 agent 卡住/失败/变慢”。
- 生态接入面广，说明兼容 OpenTelemetry/SDK 风格事件会有长期价值。

不直接照搬：

- 它是面向团队和服务端的观测平台，Agent Bar 更偏个人桌面实时感知。
- 对普通用户而言 trace tree 信息密度过高，需要转译为更自然的状态卡片和时间轴。

## Flowise

定位：可视化构建 AI agent / LLM workflow 的低代码平台。

值得借鉴：

- React + Node/TypeScript 生态，节点式 UI 对 agent 编排友好。
- components/nodes 的插件化接入模式适合参考。

不直接照搬：

- Agent Bar 的首屏不是 workflow builder，而是“状态条 + 当天安排 + 实时活动”。
- 节点编辑器可以作为高级面板，不应该压过常驻 bar 的轻量体验。

## Early Conclusion

Agent Bar 最像三类系统的交叉：

- ActivityWatch: 时间/活动事件模型。
- OpenHands: agent 运行过程可视化。
- Langfuse/Flowise: agent 接入、trace、编排和生态。

因此第一版应避免一开始做成“大而全 agent 平台”。更稳的 MVP 是：

1. 一个桌面常驻 bar。
2. 一个 24h timeline。
3. 一个本地 ingestion API。
4. 一个 agent activity stream。
5. 一个可展开详情面板，用来查看 trace、任务、产物和人工介入点。

## References

- ActivityWatch architecture: https://docs.activitywatch.net/en/latest/architecture.html
- ActivityWatch data model: https://docs.activitywatch.net/en/latest/buckets-and-events.html
- OpenHands repository: https://github.com/OpenHands/OpenHands
- Langfuse repository: https://github.com/langfuse/langfuse
- Flowise repository: https://github.com/FlowiseAI/Flowise
- Tauri architecture: https://v2.tauri.app/concept/architecture/
- Electron BrowserWindow API: https://www.electronjs.org/docs/latest/api/browser-window
