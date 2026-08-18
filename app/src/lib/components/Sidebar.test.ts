import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import type { CategoryDto } from "$lib/api";
import type { View } from "$lib/view";

const cats: CategoryDto[] = [
  { id: 1, name: "武器", ord: 1, mod_count: 2 },
  { id: 2, name: "光影", ord: 2, mod_count: 0 },
];

type SidebarProps = {
  view: View;
  categories: CategoryDto[];
  charCatName: string;
  allCount: number;
  charCount: number;
  uncatCount: number;
  query: string;
  onnavigate: (v: View) => void;
  onchanged: () => void;
};

function props(over: Partial<SidebarProps> = {}) {
  return {
    view: { kind: "home" } as const,
    categories: cats,
    charCatName: "角色",
    allCount: 5,
    charCount: 3,
    uncatCount: 1,
    query: "",
    onnavigate: vi.fn(),
    onchanged: vi.fn(),
    ...over,
  };
}

describe("Sidebar", () => {
  it("渲染内置条目与自定义分类及计数", () => {
    render(Sidebar, { props: props() });
    expect(screen.getByText("全部 Mod")).toBeTruthy();
    expect(screen.getByText("角色")).toBeTruthy();
    expect(screen.getByText("未分类")).toBeTruthy();
    expect(screen.getByText("武器")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
  });

  it("点击条目导航", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("全部 Mod"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "all" });
    await fireEvent.click(screen.getByText("武器"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "category", id: 1, name: "武器" });
  });

  it("当前视图高亮", () => {
    render(Sidebar, { props: props({ view: { kind: "category", id: 1, name: "武器" } }) });
    const btn = screen.getByText("武器").closest("button")!;
    expect(btn.getAttribute("aria-current")).toBe("page");
  });

  it("新建分类行内输入并提交", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("＋ 新建分类"));
    const input = screen.getByLabelText("新分类名称");
    await fireEvent.input(input, { target: { value: "UI" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(p.onchanged).toHaveBeenCalled();
  });

  it("分类菜单删除需二次确认", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByLabelText("分类操作 武器"));
    await fireEvent.click(screen.getByText("删除"));
    expect(screen.getByText("确认删除（2 个 Mod 移回）")).toBeTruthy();
    await fireEvent.click(screen.getByText("确认删除（2 个 Mod 移回）"));
    expect(p.onchanged).toHaveBeenCalled();
  });
});
