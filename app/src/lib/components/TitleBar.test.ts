import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import TitleBar from "./TitleBar.svelte";

describe("TitleBar", () => {
  it("按 macOS 红黄绿顺序呈现窗口控制并保留无 Tauri 渲染能力", () => {
    const { container } = render(TitleBar);
    const controls = container.querySelectorAll(".window-control");

    expect(controls).toHaveLength(3);
    expect(controls[0]).toHaveClass("window-control-close");
    expect(controls[1]).toHaveClass("window-control-minimize");
    expect(controls[2]).toHaveClass("window-control-maximize");
    expect(screen.getByRole("button", { name: "关闭" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "最小化" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "最大化" })).toBeTruthy();
    expect(screen.getByText("LiquiMod").closest(".window-title")).toBeTruthy();
    expect(container.querySelector(".window-titlebar")).toHaveAttribute("data-tauri-drag-region", "deep");
    expect(container.querySelector(".window-controls")).toHaveAttribute("data-tauri-drag-region", "false");
  });
});
