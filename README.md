# Agent Bar

Agent Bar 是一个面向桌面常驻的 agent 工作可视化与时间安排可视化项目。

目标不是先做一个聊天窗口，而是做一个能贴近日常工作流的桌面状态条：

- 用 24 小时时间轴展示今日/本周安排、计划块、会议、学习和深度工作时段。
- 用实时活动流展示正在运行的 agent、任务状态、工具调用、产物和异常。
- 提供开放接入协议，让不同 agent 可以把状态、计划、行动和结果写入同一个可视层。
- 长期目标是让人能一眼看懂“今天要做什么、agent 正在帮我做什么、下一步该不该介入”。

## Current Stage

当前仓库处于想法收敛和技术选型阶段，暂不急于实现完整应用。第一步先沉淀调研、需求边界和候选架构。

## Research Notes

- [docs/research-notes.md](docs/research-notes.md): 相近开源项目观察。
- [docs/architecture-sketch.md](docs/architecture-sketch.md): 初步架构草图和技术候选。

## Initial Direction

初步倾向：

- 桌面壳：Tauri 2 + React/TypeScript。
- 前端状态：Zustand 或 TanStack Store。
- 时间轴：自研 24h/weekly timeline 组件，数据模型参考 ActivityWatch 的 event/bucket/heartbeat 思路。
- Agent 可视化：事件流 + trace graph + 当前任务面板。
- 接入协议：先从本地 WebSocket/HTTP ingestion 开始，后续兼容 OpenTelemetry 或 MCP 风格 tool events。

## Repo Status

This repository is intentionally lightweight for now. Implementation will begin after the product shape and integration protocol are clearer.
