import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import InstallOverlay from "./InstallOverlay.svelte";
import type { InstallJob } from "$lib/install.svelte";
import type { CategoryDto, CharacterSummary } from "$lib/api";

const characters: CharacterSummary[] = [
  { internal_name: "Firefly", display_name: "Firefly", image: "firefly.png", total: 1, enabled: 0 },
];

const categories: CategoryDto[] = [
  { id: 1, name: "光锥", ord: 1, kind: "lightcone", mod_count: 0 },
  { id: 2, name: "立绘", ord: 2, kind: "portrait", mod_count: 0 },
  { id: 3, name: "场景", ord: 3, kind: "scene", mod_count: 0 },
  { id: 4, name: "NPC", ord: 4, kind: "npc", mod_count: 0 },
  { id: 5, name: "其他", ord: 5, kind: "other", mod_count: 0 },
];

function job(partial: Partial<InstallJob>): InstallJob {
  return {
    id: 1,
    fileName: "Cool.zip",
    path: "C:/dl/Cool.zip",
    stage: "installing",
    character: null,
    modId: null,
    message: null,
    warnings: [],
    busy: false,
    ...partial,
  };
}

const baseProps = (over: Record<string, unknown> = {}) => ({
  jobs: [job({})],
  characters,
  categories,
  onInstalled: vi.fn(),
  ...over,
});

describe("InstallOverlay", () => {
  it("shows installing stage", () => {
    render(InstallOverlay, { props: baseProps() });
    expect(screen.getByText("Cool.zip")).toBeTruthy();
    expect(screen.getByText(/正在安装/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "关闭" })).toBeTruthy();
  });

  it("shows done stage with character display name and undo", () => {
    render(
      InstallOverlay,
      { props: baseProps({ jobs: [job({ stage: "done", character: "Firefly", modId: 5 })] }) },
    );
    expect(screen.getByText(/已安装到 Firefly/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "撤销" })).toBeTruthy();
  });

  it("shows password input when needed and submits", async () => {
    render(
      InstallOverlay,
      { props: baseProps({ jobs: [job({ stage: "needs-password" })] }) },
    );
    const input = screen.getByPlaceholderText("压缩包密码");
    await fireEvent.input(input, { target: { value: "pw" } });
    await fireEvent.click(screen.getByRole("button", { name: "确认" }));
    expect(input).toBeTruthy();
  });

  it("shows error message with retry", () => {
    render(
      InstallOverlay,
      { props: baseProps({ jobs: [job({ stage: "error", message: "已存在同名 Mod：Cool" })] }) },
    );
    expect(screen.getByText("已存在同名 Mod：Cool")).toBeTruthy();
    expect(screen.getByRole("button", { name: "重试" })).toBeTruthy();
  });

  it("pick-category stage shows the five fixed types", () => {
    render(
      InstallOverlay,
      { props: baseProps({ jobs: [job({ stage: "pick-category" })] }) },
    );
    expect(screen.getByText("角色")).toBeTruthy();
    expect(screen.getByText("光锥")).toBeTruthy();
    expect(screen.getByText("立绘")).toBeTruthy();
    expect(screen.getByText("场景")).toBeTruthy();
    expect(screen.getByText("NPC")).toBeTruthy();
    expect(screen.getByText("其他")).toBeTruthy();
    expect(screen.getByRole("button", { name: "安装" })).toBeTruthy();
  });

  it("renders nothing when no jobs", () => {
    const { container } = render(
      InstallOverlay,
      { props: baseProps({ jobs: [] }) },
    );
    expect(container.querySelector(".install-overlay")).toBeNull();
  });
});
