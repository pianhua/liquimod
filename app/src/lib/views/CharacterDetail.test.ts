import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CharacterDetail from "./CharacterDetail.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { CharacterSummary } from "$lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const character: CharacterSummary = {
  internal_name: "Acheron",
  display_name: "Acheron",
  image: "acheron.png",
  total: 1,
  enabled: 0,
};

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
  (window as any).__TAURI_INTERNALS__ = {};
});

describe("CharacterDetail", () => {
  it("加载并渲染 Mod 列表", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 7, name: "Summer Skin", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getAllByText("Summer Skin").length).toBeGreaterThan(0));
    expect(mockedInvoke).toHaveBeenCalledWith("list_mods", { character: "Acheron", categoryId: null });
  });

  it("点击开关调用 set_mod_enabled 并更新状态", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 7, name: "Summer Skin", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getAllByRole("switch").length).toBeGreaterThan(0));
    mockedInvoke.mockResolvedValue(undefined);
    const switches = screen.getAllByRole("switch");
    await fireEvent.click(switches[0]);
    expect(mockedInvoke).toHaveBeenCalledWith("set_mod_enabled", { id: 7, enabled: true });
    await waitFor(() =>
      expect(screen.getAllByRole("switch")[0].getAttribute("aria-checked")).toBe("true"),
    );
  });

  it("默认自动选中第一个 Mod 并展示右侧大图与元数据", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 11, name: "First Mod", enabled: true, installed_at: 1700000000, size_bytes: 1048576, file_count: 5, path: "C:/mods/1" },
      { id: 12, name: "Second Mod", enabled: false, installed_at: 1700000000, size_bytes: 2048576, file_count: 8, path: "C:/mods/2" },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getAllByText("First Mod").length).toBeGreaterThanOrEqual(1));
    expect(screen.getByText("占用体积")).toBeTruthy();
    expect(screen.getByText("文件数量")).toBeTruthy();
    expect(screen.getByText("5 个文件")).toBeTruthy();
  });

  it("未配置 mods_dir 时显示配置提示", async () => {
    mockedInvoke.mockResolvedValue([]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: false,
      onback: () => {},
      onconfigured: () => {},
    });
    expect(screen.getByText("选择目录")).toBeTruthy();
  });

  it("支持 query 属性过滤 Mod 列表", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 11, name: "Summer Bikini", enabled: true, installed_at: 1700000000 },
      { id: 12, name: "Winter Coat", enabled: false, installed_at: 1700000000 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      query: "Winter",
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getAllByText("Winter Coat").length).toBeGreaterThanOrEqual(1));
    expect(screen.queryByText("Summer Bikini")).toBeNull();
  });

  it("多个 Mod 启用时显示风险提示且不再提供互斥模式", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 21, name: "Mod A", enabled: true, installed_at: 1 },
      { id: 22, name: "Mod B", enabled: true, installed_at: 2 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      warnMultipleEnabled: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getByText(/2 个 Mod 同时启用/)).toBeTruthy());
    expect(screen.queryByText("单选互斥换装")).toBeNull();
  });

  it("游戏运行期间保留启停但禁用文件变更操作", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 31, name: "Runtime Mod", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      gameRunning: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getAllByRole("switch").length).toBeGreaterThan(0));
    expect(screen.getByRole("button", { name: "导入压缩包" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getAllByRole("switch")[0].hasAttribute("disabled")).toBe(false);
    expect(screen.getByRole("button", { name: "重命名 Runtime Mod" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "卸载 Runtime Mod" }).hasAttribute("disabled")).toBe(true);
  });

  it("支持在卡片网格与列表视图之间切换", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 101, name: "Gallery Mod", enabled: true, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getByRole("region", { name: "Mod 卡片网格" })).toBeDefined());
    const listBtn = screen.getByRole("button", { name: "列表视图" });
    await fireEvent.click(listBtn);
    expect(screen.getByRole("region", { name: "Mod 列表" })).toBeDefined();
    const gridBtn = screen.getByRole("button", { name: "卡片网格视图" });
    await fireEvent.click(gridBtn);
    expect(screen.getByRole("region", { name: "Mod 卡片网格" })).toBeDefined();
  });

  it("支持折叠和展开右侧详情栏", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 102, name: "Collapsible Mod", enabled: true, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      categories: [],
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getByRole("button", { name: "收起详情" })).toBeDefined());
    expect(screen.getByRole("separator", { name: "拖拽调节详情面板宽度" })).toBeDefined();
    await fireEvent.click(screen.getByRole("button", { name: "收起详情" }));
    expect(screen.queryByRole("separator", { name: "拖拽调节详情面板宽度" })).toBeNull();
    expect(screen.getByRole("button", { name: "展开详情" })).toBeDefined();
    await fireEvent.click(screen.getByRole("button", { name: "展开详情" }));
    expect(screen.getByRole("separator", { name: "拖拽调节详情面板宽度" })).toBeDefined();
  });
});
