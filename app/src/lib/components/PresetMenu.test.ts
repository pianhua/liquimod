import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import PresetMenu from "./PresetMenu.svelte";
import { api, type PresetDto } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPresets: vi.fn(),
      savePreset: vi.fn(),
      applyPreset: vi.fn(),
      deletePreset: vi.fn(),
    },
  };
});

const presets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1 },
  { id: 2, name: "截图模式", created_at: 2 },
];

describe("PresetMenu", () => {
  beforeEach(() => {
    vi.mocked(api.listPresets).mockResolvedValue(presets);
    vi.mocked(api.savePreset).mockResolvedValue({ id: 3, name: "新", created_at: 3 });
    vi.mocked(api.applyPreset).mockResolvedValue({ enabled: 2, disabled: 1 });
    vi.mocked(api.deletePreset).mockResolvedValue(undefined);
  });

  it("打开时加载并列出预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => expect(screen.getByText("日常出战")).toBeTruthy());
    expect(screen.getByText("截图模式")).toBeTruthy();
  });

  it("保存当前为预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await fireEvent.input(screen.getByPlaceholderText("保存当前启用为预设…"), {
      target: { value: "新组合" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(api.savePreset).toHaveBeenCalledWith("新组合");
  });

  it("应用预设并回调 onapplied", async () => {
    const onapplied = vi.fn();
    render(PresetMenu, { props: { onapplied } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => screen.getByText("日常出战"));
    await fireEvent.click(screen.getByText("日常出战"));
    expect(api.applyPreset).toHaveBeenCalledWith(1, "日常出战");
    await waitFor(() => expect(onapplied).toHaveBeenCalled());
  });

  it("应用失败也回调 onapplied（部分应用后刷新列表）", async () => {
    vi.mocked(api.applyPreset).mockRejectedValueOnce(new Error("部分 Mod 应用失败"));
    const onapplied = vi.fn();
    render(PresetMenu, { props: { onapplied } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => screen.getByText("日常出战"));
    await fireEvent.click(screen.getByText("日常出战"));
    await waitFor(() => expect(onapplied).toHaveBeenCalled());
  });

  it("删除预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => screen.getByText("日常出战"));
    await fireEvent.click(screen.getByLabelText("删除预设 日常出战"));
    expect(api.deletePreset).toHaveBeenCalledWith(1);
  });
});