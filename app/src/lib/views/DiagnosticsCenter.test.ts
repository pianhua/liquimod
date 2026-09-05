import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DiagnosticsCenter from "./DiagnosticsCenter.svelte";
import { api, type DiagnosticsCenterDto } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("$lib/api")>();
  return {
    ...original,
    api: {
      ...original.api,
      getDiagnosticsCenter: vi.fn(),
      repairDeployment: vi.fn(),
    },
  };
});

const report: DiagnosticsCenterDto = {
  environment: {
    helper_ready: true,
    game_configured: true,
    loader_configured: false,
    mods_dir_configured: true,
    checks: [
      { id: "library", label: "LiquiMod 仓库", state: "pass", detail: "目录可用", remediation: null },
      { id: "mods_dir", label: "3Dmigoto Mods 目录", state: "warn", detail: "需要关注", remediation: "检查路径" },
    ],
    filesystem: "NTFS",
    deploy_strategy: "NTFS 极速软链接模式",
    defender_command: null,
  },
  deployment: {
    configured: true,
    strategy: "NTFS 极速软链接模式",
    filesystem: "NTFS",
    total_mods: 3,
    enabled_mods: 2,
    healthy_mods: 1,
    attention_mods: 2,
  },
  mods: [
    {
      id: 1,
      character: "Firefly",
      name: "Stable Outfit",
      enabled: true,
      storage_kind: "managed",
      source_available: true,
      deployment_state: "deployed",
      detail: "数据库状态与磁盘 Junction 部署一致",
    },
    {
      id: 2,
      character: "Acheron",
      name: "Offline Source",
      enabled: true,
      storage_kind: "external",
      source_available: false,
      deployment_state: "source_unavailable",
      detail: "源目录不可用，无法验证或恢复部署",
    },
    {
      id: 3,
      character: "Firefly",
      name: "Disabled Mod",
      enabled: false,
      storage_kind: "managed",
      source_available: true,
      deployment_state: "disabled",
      detail: "Mod 已禁用，未检查到活动部署",
    },
  ],
  hash_conflicts: [
    { hash: "abc123", section: "TextureOverride", conflicting_mods: [{ id: 1, character: "Firefly", name: "Stable Outfit" }, { id: 2, character: "Acheron", name: "Offline Source" }] },
  ],
  variable_conflicts: [],
};

describe("DiagnosticsCenter", () => {
  beforeEach(() => {
    vi.mocked(api.getDiagnosticsCenter).mockResolvedValue(structuredClone(report));
    vi.mocked(api.repairDeployment).mockResolvedValue(undefined);
    vi.mocked(api.getDiagnosticsCenter).mockClear();
    vi.mocked(api.repairDeployment).mockClear();
  });

  it("展示部署摘要、环境检查和 Mod 状态", async () => {
    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged: vi.fn() } });

    await waitFor(() => expect(screen.getByText("Stable Outfit")).toBeTruthy());
    expect(screen.getByText("Mod 状态与诊断中心")).toBeTruthy();
    expect(screen.getByText("源离线")).toBeTruthy();
    expect(screen.getByText("Hash 冲突")).toBeTruthy();
    expect(screen.getByText("abc123")).toBeTruthy();
  });

  it("可以只查看需要处理的 Mod", async () => {
    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged: vi.fn() } });
    await waitFor(() => expect(screen.getByText("Stable Outfit")).toBeTruthy());

    await fireEvent.click(screen.getByRole("button", { name: "需处理" }));
    expect(screen.queryByText("Stable Outfit")).toBeNull();
    expect(screen.getByText("Offline Source")).toBeTruthy();
  });

  it("修复部署后重新读取并通知父视图", async () => {
    const onchanged = vi.fn();
    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged } });
    await waitFor(() => expect(screen.getByText("Stable Outfit")).toBeTruthy());

    await fireEvent.click(screen.getByRole("button", { name: "修复部署" }));
    await waitFor(() => expect(api.repairDeployment).toHaveBeenCalledTimes(1));
    expect(onchanged).toHaveBeenCalledTimes(1);
  });

  it("检查失败时允许重新尝试", async () => {
    vi.mocked(api.getDiagnosticsCenter)
      .mockRejectedValueOnce(new Error("temporary failure"))
      .mockResolvedValueOnce(structuredClone(report));
    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged: vi.fn() } });

    await waitFor(() => expect(screen.getByText("诊断读取失败")).toBeTruthy());
    await fireEvent.click(screen.getByRole("button", { name: "再试一次" }));
    await waitFor(() => expect(screen.getByText("Stable Outfit")).toBeTruthy());
    expect(api.getDiagnosticsCenter).toHaveBeenCalledTimes(2);
  });
});
