import { render, screen, fireEvent, act } from "@testing-library/svelte";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import TooltipRoot from "./TooltipRoot.svelte";
import { parseTooltipContent } from "$lib/tooltip";

describe("Tooltip 系统", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("parseTooltipContent 正确提取快捷键", () => {
    expect(parseTooltipContent("打开目录")).toEqual({ main: "打开目录" });
    expect(parseTooltipContent("取消 (Esc)")).toEqual({ main: "取消", shortcut: "Esc" });
    expect(parseTooltipContent("搜索角色或 Mod (Ctrl+K)")).toEqual({
      main: "搜索角色或 Mod",
      shortcut: "Ctrl+K",
    });
    expect(parseTooltipContent("缩小 （-）")).toEqual({ main: "缩小", shortcut: "-" });
  });

  it("挂载 TooltipRoot 并响应悬停事件", async () => {
    // 渲染宿主 DOM
    document.body.innerHTML = `
      <button id="test-btn" title="偏好设置 (Ctrl+,)">设置</button>
    `;
    const btn = document.getElementById("test-btn")!;

    render(TooltipRoot);

    // 触发 pointerover
    await fireEvent.pointerOver(btn);
    // 等待 200ms 延迟
    await act(() => {
      vi.advanceTimersByTime(250);
    });

    const tip = screen.getByRole("tooltip");
    expect(tip).toBeTruthy();
    expect(tip.textContent).toContain("偏好设置");
    expect(tip.textContent).toContain("Ctrl+,");

    // 原生 title 属性被清除（防止系统原生黑框）
    expect(btn.getAttribute("title")).toBeNull();
    expect(btn.getAttribute("data-liquimod-tip")).toBe("偏好设置 (Ctrl+,)");

    // 触发 pointerOut
    await fireEvent.pointerOut(btn);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });
});
