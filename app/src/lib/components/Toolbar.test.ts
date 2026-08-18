import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Toolbar from "./Toolbar.svelte";

function props(over: Record<string, unknown> = {}) {
  return {
    crumbs: ["角色", "流萤"],
    sort: "recent" as const,
    showSort: true,
    onlaunchgame: vi.fn(),
    onlaunchloader: vi.fn(),
    ...over,
  };
}

describe("Toolbar", () => {
  it("渲染面包屑与两个启动按钮", () => {
    render(Toolbar, { props: props() });
    expect(screen.getByLabelText("面包屑").textContent).toContain("角色");
    expect(screen.getByText("启动游戏")).toBeTruthy();
    expect(screen.getByText("启动加载器")).toBeTruthy();
  });

  it("启动按钮回调", async () => {
    const p = props();
    render(Toolbar, { props: p });
    await fireEvent.click(screen.getByText("启动游戏"));
    expect(p.onlaunchgame).toHaveBeenCalled();
    await fireEvent.click(screen.getByText("启动加载器"));
    expect(p.onlaunchloader).toHaveBeenCalled();
  });

  it("showSort=false 时不渲染排序", () => {
    render(Toolbar, { props: props({ showSort: false }) });
    expect(screen.queryByLabelText("排序方式")).toBeNull();
  });
});