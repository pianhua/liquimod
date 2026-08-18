import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import type { CategoryDto } from "$lib/api";
import type { View } from "$lib/view";

const cats: CategoryDto[] = [
  { id: 1, name: "光锥", ord: 1, kind: "lightcone", mod_count: 2 },
  { id: 2, name: "立绘", ord: 2, kind: "portrait", mod_count: 0 },
  { id: 3, name: "场景", ord: 3, kind: "scene", mod_count: 0 },
  { id: 4, name: "NPC", ord: 4, kind: "npc", mod_count: 1 },
  { id: 5, name: "其他", ord: 5, kind: "other", mod_count: 0 },
  { id: 9, name: "武器", ord: 6, kind: null, mod_count: 3 },
];

type SidebarProps = {
  view: View;
  categories: CategoryDto[];
  charCatName: string;
  charCount: number;
  query: string;
  onnavigate: (v: View) => void;
  onchanged: () => void;
  onapplied: () => void;
};

function props(over: Partial<SidebarProps> = {}) {
  return {
    view: { kind: "home" } as const,
    categories: cats,
    charCatName: "角色",
    charCount: 3,
    query: "",
    onnavigate: vi.fn(),
    onchanged: vi.fn(),
    onapplied: vi.fn(),
    ...over,
  };
}

describe("Sidebar", () => {
  it("渲染固定六类导航（空类也显示），自定义分类不显示", () => {
    render(Sidebar, { props: props() });
    expect(screen.getByText("角色")).toBeTruthy();
    expect(screen.getByText("光锥")).toBeTruthy();
    expect(screen.getByText("立绘")).toBeTruthy();
    expect(screen.getByText("场景")).toBeTruthy();
    expect(screen.getByText("NPC")).toBeTruthy();
    expect(screen.getByText("其他")).toBeTruthy();
    // 自定义分类「武器」不再展示为导航
    expect(screen.queryByText("武器")).toBeNull();
    // 旧入口已移除
    expect(screen.queryByText("全部 Mod")).toBeNull();
    expect(screen.queryByText("未分类")).toBeNull();
    expect(screen.queryByText("＋ 新建分类")).toBeNull();
  });

  it("显示各类计数", () => {
    render(Sidebar, { props: props() });
    // 光锥 2、NPC 1、角色 3（charCount）
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
  });

  it("点击角色导航到 home", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("角色"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "home" });
  });

  it("点击实体类导航到 type", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("光锥"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "type", id: 1, name: "光锥" });
  });

  it("当前 type 视图高亮", () => {
    render(Sidebar, { props: props({ view: { kind: "type", id: 1, name: "光锥" } }) });
    const btn = screen.getByText("光锥").closest("button")!;
    expect(btn.getAttribute("aria-current")).toBe("page");
  });
});