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
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getByText("Summer Skin")).toBeTruthy());
    expect(mockedInvoke).toHaveBeenCalledWith("list_mods", { character: "Acheron" });
  });

  it("点击开关调用 set_mod_enabled 并更新状态", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 7, name: "Summer Skin", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => screen.getByRole("switch"));
    mockedInvoke.mockResolvedValue(undefined);
    await fireEvent.click(screen.getByRole("switch"));
    expect(mockedInvoke).toHaveBeenCalledWith("set_mod_enabled", { id: 7, enabled: true });
    await waitFor(() =>
      expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("true"),
    );
  });

  it("未配置 mods_dir 时显示配置提示", async () => {
    mockedInvoke.mockResolvedValue([]);
    render(CharacterDetail, {
      character,
      modsDirConfigured: false,
      onback: () => {},
      onconfigured: () => {},
    });
    expect(screen.getByText("选择目录")).toBeTruthy();
  });
});