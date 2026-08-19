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

  it("存在冲突时展示冲突诊断徽章并可点击查看", async () => {
    const mockConflicts = [
      {
        hash: "9de39691",
        section: "TextureOverrideBody",
        conflicting_mods: [
          { id: 1, character: "Acheron", name: "ModA" },
          { id: 2, character: "Acheron", name: "ModB" },
        ],
      },
    ];
    render(Toolbar, { props: props({ conflicts: mockConflicts }) });
    const badge = screen.getByText("1 处冲突");
    expect(badge).toBeTruthy();
    await fireEvent.click(badge);
    expect(screen.getByText("Mod 覆盖冲突诊断 (1)")).toBeTruthy();
    expect(screen.getByText("Hash: 9de39691")).toBeTruthy();
  });
});
