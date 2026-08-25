import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Toolbar from "./Toolbar.svelte";

function props(over: Record<string, unknown> = {}) {
  return {
    crumbs: ["角色", "流萤"],
    sort: "recent" as const,
    showSort: true,
    onlaunchmodgame: vi.fn(),
    onlaunchnativegame: vi.fn(),
    onrefreshgame: vi.fn(),
    ontogglesettings: vi.fn(),
    onapplied: vi.fn(),
    ...over,
  };
}

describe("Toolbar", () => {
  it("渲染面包屑与两个启动按钮", () => {
    render(Toolbar, { props: props() });
    expect(screen.getByLabelText("面包屑").textContent).toContain("角色");
    expect(screen.getByText("模组启动")).toBeTruthy();
    expect(screen.getByText("纯净启动")).toBeTruthy();
  });

  it("启动按钮回调", async () => {
    const p = props();
    render(Toolbar, { props: p });
    await fireEvent.click(screen.getByText("模组启动"));
    expect(p.onlaunchmodgame).toHaveBeenCalled();
    await fireEvent.click(screen.getByText("纯净启动"));
    expect(p.onlaunchnativegame).toHaveBeenCalled();
  });

  it("渲染并点击刷新按钮", async () => {
    const p = props({ onrefreshgame: vi.fn() });
    render(Toolbar, { props: p });
    const btn = screen.getByText("热重载");
    expect(btn).toBeTruthy();
    await fireEvent.click(btn);
    expect(p.onrefreshgame).toHaveBeenCalled();
  });

  it("showSort=false 时不渲染排序", () => {
    render(Toolbar, { props: props({ showSort: false }) });
    expect(screen.queryByLabelText("排序方式")).toBeNull();
  });

  it("游戏运行时显示状态并保留手动热重载", () => {
    render(Toolbar, { props: props({ gameRunning: true }) });
    expect(screen.getByText("游戏运行中")).toBeTruthy();
    expect(screen.getByText("热重载")).toBeTruthy();
  });
});
