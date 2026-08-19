import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";

describe("ContextMenu", () => {
  it("渲染菜单项与快捷键", () => {
    const items: MenuItem[] = [
      { id: "1", label: "启用此 Mod", shortcut: "Space" },
      { id: "2", label: "在资源管理器中定位", icon: "📂" },
      { id: "d1", label: "", divider: true },
      { id: "3", label: "卸载此 Mod", danger: true, shortcut: "Del" },
    ];
    render(ContextMenu, { props: { x: 100, y: 100, items, onclose: () => {} } });

    expect(screen.getByText("启用此 Mod")).toBeInTheDocument();
    expect(screen.getByText("Space")).toBeInTheDocument();
    expect(screen.getByText("在资源管理器中定位")).toBeInTheDocument();
    expect(screen.getByText("卸载此 Mod")).toBeInTheDocument();
    expect(screen.getByText("Del")).toBeInTheDocument();
  });

  it("点击菜单项触发 action 并关闭", async () => {
    const action = vi.fn();
    const onclose = vi.fn();
    const items: MenuItem[] = [
      { id: "1", label: "启用此 Mod", action },
    ];
    render(ContextMenu, { props: { x: 100, y: 100, items, onclose } });

    await fireEvent.click(screen.getByText("启用此 Mod"));
    expect(action).toHaveBeenCalledTimes(1);
    expect(onclose).toHaveBeenCalledTimes(1);
  });

  it("按 Escape 键触发 onclose", async () => {
    const onclose = vi.fn();
    render(ContextMenu, { props: { x: 100, y: 100, items: [{ id: "1", label: "测试" }], onclose } });

    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
  });
});
