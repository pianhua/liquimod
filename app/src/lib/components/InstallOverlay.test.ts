import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import InstallOverlay from "./InstallOverlay.svelte";
import type { InstallJob } from "$lib/install.svelte";
import type { CharacterSummary } from "$lib/api";

const characters: CharacterSummary[] = [
  { internal_name: "Firefly", display_name: "Firefly", image: "firefly.png", total: 1, enabled: 0 },
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
    ...partial,
  };
}

describe("InstallOverlay", () => {
  it("shows installing stage", () => {
    render(InstallOverlay, { props: { jobs: [job({})], characters, onInstalled: vi.fn() } });
    expect(screen.getByText("Cool.zip")).toBeTruthy();
    expect(screen.getByText(/正在安装/)).toBeTruthy();
  });

  it("shows done stage with character display name and undo", () => {
    render(InstallOverlay, {
      props: {
        jobs: [job({ stage: "done", character: "Firefly", modId: 5 })],
        characters,
        onInstalled: vi.fn(),
      },
    });
    expect(screen.getByText(/已安装到 Firefly/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "撤销" })).toBeTruthy();
  });

  it("shows password input when needed and submits", async () => {
    render(InstallOverlay, {
      props: { jobs: [job({ stage: "needs-password" })], characters, onInstalled: vi.fn() },
    });
    const input = screen.getByPlaceholderText("压缩包密码");
    await fireEvent.input(input, { target: { value: "pw" } });
    await fireEvent.click(screen.getByRole("button", { name: "确认" }));
    expect(input).toBeTruthy();
  });

  it("shows error message with retry", () => {
    render(InstallOverlay, {
      props: {
        jobs: [job({ stage: "error", message: "已存在同名 Mod：Cool" })],
        characters,
        onInstalled: vi.fn(),
      },
    });
    expect(screen.getByText("已存在同名 Mod：Cool")).toBeTruthy();
    expect(screen.getByRole("button", { name: "重试" })).toBeTruthy();
  });

  it("renders nothing when no jobs", () => {
    const { container } = render(InstallOverlay, {
      props: { jobs: [], characters, onInstalled: vi.fn() },
    });
    expect(container.querySelector(".install-overlay")).toBeNull();
  });
});