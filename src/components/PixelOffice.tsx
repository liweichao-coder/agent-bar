import { useEffect, useRef } from "react";
import type { Agent } from "../types";

type PixelOfficeProps = {
  agents: Agent[];
  selectedId: string;
  onSelect: (id: string) => void;
};

const stateLabels = {
  working: "编写中",
  searching: "检索中",
  waiting: "等待批准",
  recent: "最近活动",
  idle: "空闲",
  blocked: "已阻塞",
};

function drawPixelAgent(
  context: CanvasRenderingContext2D,
  agent: Agent,
  width: number,
  height: number,
  tick: number,
  selected: boolean,
) {
  const x = (agent.position.x / 100) * width;
  const y = (agent.position.y / 100) * height;
  const bob = agent.status === "idle" ? 0 : Math.round(Math.sin(tick / 280 + x) * 2);
  const scale = Math.max(1, Math.min(1.45, width / 520));
  const px = Math.round(4 * scale);

  if (selected) {
    context.fillStyle = "rgba(255, 255, 255, 0.16)";
    context.fillRect(x - px * 6, y - px * 7, px * 12, px * 11);
  }

  context.fillStyle = "rgba(0, 0, 0, 0.24)";
  context.fillRect(x - px * 4, y + px * 3, px * 8, px * 2);

  context.fillStyle = agent.accent;
  context.fillRect(x - px * 3, y - px * 3 + bob, px * 6, px * 6);
  context.fillStyle = "#f7d4b1";
  context.fillRect(x - px * 2, y - px * 6 + bob, px * 4, px * 3);
  context.fillStyle = "#242520";
  context.fillRect(x - px * 2, y - px * 7 + bob, px * 4, px);
  context.fillRect(x - px * 3, y - px * 6 + bob, px, px * 2);
  context.fillStyle = "#20211e";
  context.fillRect(x - px, y - px * 5 + bob, px, px);
  context.fillRect(x + px, y - px * 5 + bob, px, px);
  context.fillStyle = "#34352f";
  context.fillRect(x - px * 3, y + px * 3 + bob, px * 2, px * 3);
  context.fillRect(x + px, y + px * 3 + bob, px * 2, px * 3);

  if (agent.status === "working" || agent.status === "searching" || agent.status === "recent") {
    context.fillStyle = agent.status === "working" ? "#74d7a6" : agent.status === "searching" ? "#79baff" : "#e7c66d";
    const blink = Math.floor(tick / 360) % 3;
    context.fillRect(x + px * 4, y - px * (4 + blink), px, px);
    context.fillRect(x + px * 6, y - px * (6 - blink), px, px);
  }

  if (agent.status === "waiting") {
    context.fillStyle = "#ff9274";
    context.fillRect(x + px * 4, y - px * 8, px * 3, px * 3);
    context.fillStyle = "#242520";
    context.fillRect(x + px * 5, y - px * 7, px, px);
  }
}

function drawOffice(context: CanvasRenderingContext2D, width: number, height: number, agents: Agent[], selectedId: string, tick: number) {
  context.clearRect(0, 0, width, height);
  context.fillStyle = "#20211d";
  context.fillRect(0, 0, width, height);
  context.fillStyle = "#2b2c26";
  context.fillRect(0, height * 0.26, width, height * 0.74);

  const tile = Math.max(18, Math.round(width / 24));
  context.strokeStyle = "#34362f";
  context.lineWidth = 1;
  for (let x = 0; x < width; x += tile) {
    context.beginPath();
    context.moveTo(x, height * 0.26);
    context.lineTo(x, height);
    context.stroke();
  }
  for (let y = height * 0.26; y < height; y += tile) {
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(width, y);
    context.stroke();
  }

  context.fillStyle = "#171815";
  context.fillRect(width * 0.07, height * 0.08, width * 0.2, height * 0.12);
  context.fillRect(width * 0.36, height * 0.08, width * 0.2, height * 0.12);
  context.fillStyle = "#77acd0";
  context.fillRect(width * 0.08, height * 0.09, width * 0.18, height * 0.09);
  context.fillRect(width * 0.37, height * 0.09, width * 0.18, height * 0.09);
  context.fillStyle = "#c9d8d1";
  context.fillRect(width * 0.08, height * 0.135, width * 0.18, 2);
  context.fillRect(width * 0.46, height * 0.09, 2, height * 0.09);

  const desks = [[0.17, 0.49], [0.41, 0.28], [0.65, 0.28], [0.72, 0.59]];
  for (const [dx, dy] of desks) {
    context.fillStyle = "#8f674d";
    context.fillRect(width * dx, height * dy, width * 0.17, height * 0.06);
    context.fillStyle = "#151614";
    context.fillRect(width * (dx + 0.055), height * (dy - 0.055), width * 0.065, height * 0.06);
    context.fillStyle = "#78d3a7";
    context.fillRect(width * (dx + 0.061), height * (dy - 0.046), width * 0.053, height * 0.034);
  }

  context.fillStyle = "#4f6f4e";
  context.fillRect(width * 0.88, height * 0.34, width * 0.06, height * 0.18);
  context.fillStyle = "#7bc487";
  context.fillRect(width * 0.85, height * 0.27, width * 0.06, height * 0.1);
  context.fillRect(width * 0.91, height * 0.25, width * 0.05, height * 0.12);
  context.fillStyle = "#be735f";
  context.fillRect(width * 0.86, height * 0.49, width * 0.09, height * 0.07);

  for (const agent of agents) {
    drawPixelAgent(context, agent, width, height, tick, agent.id === selectedId);
  }
}

export function PixelOffice({ agents, selectedId, onSelect }: PixelOfficeProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const context = canvas.getContext("2d");
    if (!context) return;

    let frame = 0;
    let visible = !document.hidden;
    let width = 0;
    let height = 0;

    const resize = () => {
      const bounds = container.getBoundingClientRect();
      const ratio = Math.min(window.devicePixelRatio || 1, 2);
      width = Math.max(320, Math.floor(bounds.width));
      height = Math.max(260, Math.floor(bounds.height));
      canvas.width = Math.floor(width * ratio);
      canvas.height = Math.floor(height * ratio);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.imageSmoothingEnabled = false;
    };

    const render = (tick: number) => {
      if (visible) drawOffice(context, width, height, agents, selectedId, tick);
      frame = window.requestAnimationFrame(render);
    };

    const onVisibilityChange = () => { visible = !document.hidden; };
    const intersectionObserver = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting && !document.hidden;
    }, { rootMargin: "120px" });
    const resizeObserver = new ResizeObserver(resize);

    resize();
    resizeObserver.observe(container);
    intersectionObserver.observe(container);
    document.addEventListener("visibilitychange", onVisibilityChange);
    frame = window.requestAnimationFrame(render);

    return () => {
      window.cancelAnimationFrame(frame);
      resizeObserver.disconnect();
      intersectionObserver.disconnect();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [agents, selectedId]);

  return (
    <div className="office-stage" ref={containerRef}>
      <canvas ref={canvasRef} aria-hidden="true" />
      {agents.map((agent) => (
        <button
          key={agent.id}
          type="button"
          className={`agent-hit-target ${agent.id === selectedId ? "selected" : ""}`}
          style={{ left: `${agent.position.x}%`, top: `${agent.position.y}%` }}
          onClick={() => onSelect(agent.id)}
          aria-label={`查看 ${agent.name}，当前${stateLabels[agent.status]}`}
        />
      ))}
      <div className="office-status" aria-hidden="true">
        <span className="live-pulse" /> LIVE OFFICE
      </div>
    </div>
  );
}
