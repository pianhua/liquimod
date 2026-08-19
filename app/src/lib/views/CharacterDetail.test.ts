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
});
