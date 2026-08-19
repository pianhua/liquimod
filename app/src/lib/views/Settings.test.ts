import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import Settings from "./Settings.svelte";
import { api } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPasswords: vi.fn(),
      addPassword: vi.fn(),
      removePassword: vi.fn(),
      setAutoEnable: vi.fn(),
      readLog: vi.fn(),
      getLocalAssetVersion: vi.fn(),
      checkGameAssetsUpdate: vi.fn(),
      syncGameAssets: vi.fn(),
      checkMigotoUpdate: vi.fn(),
      installMigotoUpdate: vi.fn(),
      migrateModsFromOldMigoto: vi.fn(),
    },
    isTauri: () => false,
  };
});

const config: import("$lib/api").ConfigDto = {
  library_root: "C:/mock/Library",
  mods_dir: "D:/game/Mods",
  auto_enable: false,
  theme: "auto",
  character_category_name: "角色",
  game_exe: null,
  loader_exe: null,
  work_mode: "play",
  injection_delay_ms: 500,
  github_token: "",
  github_mirror: "",
  migoto_version: "v2.4.2",
};
const testConfig: import("$lib/api").ConfigDto = {
  library_root: "C:/L",
  mods_dir: null,
  auto_enable: false,
  theme: "auto",
  character_category_name: "角色",
  game_exe: null,
  loader_exe: null,
  work_mode: "play",
  injection_delay_ms: 500,
  github_token: "",
  github_mirror: "",
  migoto_version: "v2.4.2",
};

describe("Settings", () => {
  beforeEach(() => {
    vi.mocked(api.listPasswords).mockResolvedValue(["1234"]);
    vi.mocked(api.addPassword).mockResolvedValue(undefined);
    vi.mocked(api.removePassword).mockResolvedValue(undefined);
    vi.mocked(api.setAutoEnable).mockResolvedValue(testConfig);
    vi.mocked(api.readLog).mockResolvedValue("2026-08-18T10:00:00 INFO hello log");
    vi.mocked(api.readLog).mockClear();
    vi.mocked(api.setAutoEnable).mockClear();
  });

  it("显示目录配置", () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    expect(screen.getByText("C:/mock/Library")).toBeTruthy();
    expect(screen.getByText("D:/game/Mods")).toBeTruthy();
  });

  it("加载并展示密码本", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => expect(screen.getByText("1234")).toBeTruthy());
  });

  it("添加密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await fireEvent.input(screen.getByPlaceholderText("添加解压密码…"), {
      target: { value: "abc" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "添加" }));
    expect(api.addPassword).toHaveBeenCalledWith("abc");
  });

  it("移除密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => screen.getByText("1234"));
    await fireEvent.click(screen.getByLabelText("移除密码 1234"));
    expect(api.removePassword).toHaveBeenCalledWith("1234");
  });

  it("返回回调", async () => {
    const onback = vi.fn();
    render(Settings, { props: { config, onback, onchanged: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: /返回/ }));
    expect(onback).toHaveBeenCalled();
  });

  it("自动启用开关调用 setAutoEnable", async () => {
    render(Settings, { props: { config: testConfig, onback: vi.fn(), onchanged: vi.fn() } });
    await fireEvent.click(screen.getByRole("switch", { name: "自动启用" }));
    expect(api.setAutoEnable).toHaveBeenCalledWith(true);
  });

  it("日志区加载并刷新", async () => {
    render(Settings, { props: { config: testConfig, onback: vi.fn(), onchanged: vi.fn() } });
    await waitFor(() => screen.getByText(/hello log/));
    await fireEvent.click(screen.getByText("刷新"));
    expect(api.readLog).toHaveBeenCalledTimes(2);
  });

  it("点击检查更新调用 checkGameAssetsUpdate", async () => {
    vi.mocked(api.checkGameAssetsUpdate).mockResolvedValue({
      has_update: false,
      remote_version: "v2026.08.19",
      local_version: "v2026.08.19",
    });
    render(Settings, { props: { config: testConfig, onback: vi.fn(), onchanged: vi.fn() } });
    await fireEvent.click(screen.getByText("检查更新"));
    expect(api.checkGameAssetsUpdate).toHaveBeenCalled();
  });

  it("点击同步星铁数据调用 syncGameAssets", async () => {
    vi.mocked(api.syncGameAssets).mockResolvedValue({
      success: true,
      message: "同步成功",
      version: "v2026.08.19",
      downloaded_count: 5,
      deleted_count: 0,
    });
    render(Settings, { props: { config: testConfig, onback: vi.fn(), onchanged: vi.fn() } });
    await fireEvent.click(screen.getByText("同步星铁数据"));
    expect(api.syncGameAssets).toHaveBeenCalled();
  });
});
