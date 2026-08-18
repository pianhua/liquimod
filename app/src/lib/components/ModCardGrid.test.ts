import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModCardGrid from "./ModCardGrid.svelte";
import type { ModDto } from "$lib/api";

function mod(id: number, name: string, enabled: boolean, installed_at: number, category_id: number | null = null): ModDto {
  return { id, name, enabled, installed_at, thumb: null, size_bytes: 2048, file_count: 3, path: "", category_id };
}

function props(mods: ModDto[]) {
  return {
    mods,
    categories: [{ id: 1, name: "武器", ord: 1, kind: null, mod_count: 0 }],
    sort: "recent" as const,
    query: "",
    enabledFilter: "all" as const,
    catLabelOf: (m: ModDto) => (m.category_id ? "武器" : "角色"),
    ontoggle: vi.fn(),
    onrename: vi.fn(async () => true),
    onuninstall: vi.fn(async () => {}),
    onopen: vi.fn(),
    onmove: vi.fn(),
  };
}

describe("ModCardGrid", () => {
  it("渲染卡片与副行信息", () => {
    render(ModCardGrid, { props: props([mod(1, "大剑", false, 100)]) });
    expect(screen.getByText("大剑")).toBeTruthy();
    expect(screen.getByText(/2 KB · 3 文件/)).toBeTruthy();
    expect(screen.getByText("角色")).toBeTruthy();
  });

  it("搜索过滤", () => {
    const p = props([mod(1, "大剑", false, 100), mod(2, "特效", false, 50)]);
    render(ModCardGrid, { props: { ...p, query: "特效" } });
    expect(screen.queryByText("大剑")).toBeNull();
    expect(screen.getByText("特效")).toBeTruthy();
  });

  it("空格启停", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    const card = screen.getByLabelText("大剑");
    await fireEvent.keyDown(card, { key: " " });
    expect(p.ontoggle).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }), true);
  });

  it("卸载需二次确认", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    await fireEvent.click(screen.getByLabelText("卸载 大剑"));
    // 确认提示与确认按钮均含「确认卸载」
    expect(screen.getAllByText(/确认卸载/).length).toBeGreaterThan(0);
    await fireEvent.click(screen.getByRole("button", { name: "确认卸载" }));
    expect(p.onuninstall).toHaveBeenCalled();
  });

  it("移到分类回调", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    await fireEvent.click(screen.getByLabelText("移到分类 大剑"));
    await fireEvent.click(screen.getByText("武器"));
    expect(p.onmove).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }), 1);
  });

  it("空态", () => {
    render(ModCardGrid, { props: props([]) });
    expect(screen.getByText("这里还没有 Mod")).toBeTruthy();
  });

  it("启用态筛选过滤卡片", async () => {
    const p = props([mod(1, "已启", true, 100), mod(2, "未启", false, 50)]);
    render(ModCardGrid, { props: p });
    // 默认全部
    expect(screen.getByText("已启")).toBeTruthy();
    expect(screen.getByText("未启")).toBeTruthy();
    // 点「已启用」
    await fireEvent.click(screen.getByText("已启用", { selector: "button" }));
    expect(screen.queryByText("未启")).toBeNull();
    expect(screen.getByText("已启")).toBeTruthy();
  });
});
