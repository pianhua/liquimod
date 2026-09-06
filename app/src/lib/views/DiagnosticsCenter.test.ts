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
      openModFolder: vi.fn(),
      openPathInExplorer: vi.fn(),
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
    deployment_root: "C:/mock/Game/Mods",
    pending_operations: 1,
  },
  mods: [
    {
      id: 1,
      character: "Firefly",
      name: "Stable Outfit",
      enabled: true,
      storage_kind: "managed",
      source_available: true,
      source_path: "C:/mock/Library/mods/Firefly/Stable Outfit",
      deployment_path: "C:/mock/Game/Mods/Firefly__Stable Outfit__1",
      deployment_state: "deployed",
      detail: "数据库状态与磁盘 Junction 部署一致",
      remediation: "无需处理；数据库状态与实际 Junction 一致。",
    },
    {
      id: 2,
      character: "Acheron",
      name: "Offline Source",
      enabled: true,
      storage_kind: "external",
      source_available: false,
      source_path: "C:/External/Acheron/Offline Source",
      deployment_path: "C:/mock/Game/Mods/Acheron__Offline Source__2",
      deployment_state: "source_unavailable",
      detail: "源目录不可用，无法验证或恢复部署",
      remediation: "恢复外部源目录后点击“重新检查”；源离线期间不能启用、打开或修复此 Mod，LiquiMod 不会复制或接管源文件。",
    },
    {
      id: 3,
      character: "Firefly",
      name: "Disabled Mod",
      enabled: false,
      storage_kind: "managed",
      source_available: true,
      source_path: "C:/mock/Library/mods/Firefly/Disabled Mod",
      deployment_path: "C:/mock/Game/Mods/Firefly__Disabled Mod__3",
      deployment_state: "disabled",
      detail: "Mod 已禁用，未检查到活动部署",
      remediation: "无需处理；需要使用时从资源库启用此 Mod。",
    },
  ],
  hash_conflicts: [
    { hash: "abc123", section: "TextureOverride", conflicting_mods: [{ id: 1, character: "Firefly", name: "Stable Outfit" }, { id: 2, character: "Acheron", name: "Offline Source" }] },
  ],
  variable_conflicts: [],
  pending_operations: [
    {
      id: 10,
      operation: "refresh",
      payload: "2",
      mod_id: 2,
      target: "Acheron/Offline Source",
      detail: "刷新事务（Acheron/Offline Source）尚未完成；源目录在线且路径安全时可重新修复。",
    },
  ],
};

describe("DiagnosticsCenter", () => {
  beforeEach(() => {
    vi.mocked(api.getDiagnosticsCenter).mockResolvedValue(structuredClone(report));
    vi.mocked(api.repairDeployment).mockResolvedValue({
      attempted_mods: 2,
      repaired_mods: 2,
      remaining_attention: 0,
      pending_operations: 0,
    });
    vi.mocked(api.getDiagnosticsCenter).mockClear();
    vi.mocked(api.repairDeployment).mockClear();
    vi.mocked(api.openModFolder).mockClear();
    vi.mocked(api.openPathInExplorer).mockClear();
  });

  it("展示部署摘要、环境检查和 Mod 状态", async () => {
    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged: vi.fn() } });

    await waitFor(() => expect(screen.getByText("Stable Outfit")).toBeTruthy());
    expect(screen.getByText("Mod 状态与诊断中心")).toBeTruthy();
    expect(screen.getByText("源离线")).toBeTruthy();
    expect(screen.getByText("Hash 冲突")).toBeTruthy();
    expect(screen.getByText("abc123")).toBeTruthy();
    expect(screen.getByText("待恢复操作")).toBeTruthy();
    expect(screen.getByText("C:/External/Acheron/Offline Source")).toBeTruthy();
    expect(screen.getByText(/LiquiMod 不会复制或接管源文件/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "打开 Offline Source 源目录" })).toBeNull();
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
    expect(screen.getByText("确认执行部署对账？")).toBeTruthy();
    expect(api.repairDeployment).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole("button", { name: "确认修复" }));
    await waitFor(() => expect(api.repairDeployment).toHaveBeenCalledTimes(1));
    expect(onchanged).toHaveBeenCalledTimes(1);
  });

  it("异常部署可打开 Mods 目录", async () => {
    const attentionReport = structuredClone(report);
    attentionReport.mods[1].source_available = true;
    attentionReport.mods[1].deployment_state = "mismatched";
    attentionReport.mods[1].detail = "数据库标记为启用，但 Junction 指向了错误目标";
    attentionReport.mods[1].remediation = "先检查部署入口是否被其他工具占用";
    vi.mocked(api.getDiagnosticsCenter).mockResolvedValue(attentionReport);

    render(DiagnosticsCenter, { props: { onback: vi.fn(), onchanged: vi.fn() } });
    await waitFor(() => expect(screen.getByText("Offline Source")).toBeTruthy());

    expect(screen.getByRole("button", { name: "打开 Offline Source 源目录" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "打开 Mods 目录（处理 Offline Source）" }));
    expect(api.openPathInExplorer).toHaveBeenCalledWith("C:/mock/Game/Mods");
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
