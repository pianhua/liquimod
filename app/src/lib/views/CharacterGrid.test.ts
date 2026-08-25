import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CharacterGrid from "./CharacterGrid.svelte";
import type { CharacterSummary } from "$lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const characters: CharacterSummary[] = [
  { internal_name: "Acheron", display_name: "Acheron", image: "acheron.png", total: 3, enabled: 2 },
  { internal_name: "Others", display_name: "其他", image: null, total: 1, enabled: 0 },
];

describe("CharacterGrid", () => {
  it("渲染角色卡与启用计数", () => {
    render(CharacterGrid, { characters, query: "", onselect: () => {} });
    expect(screen.getByText("Acheron")).toBeTruthy();
    expect(screen.getByText("2/3")).toBeTruthy();
    expect(screen.getByText("其他")).toBeTruthy();
  });

  it("搜索过滤后显示空态", () => {
    render(CharacterGrid, { characters, query: "zzz", onselect: () => {} });
    expect(screen.getByText("没有匹配的角色")).toBeTruthy();
  });

  it("点击卡片触发 onselect", async () => {
    const onselect = vi.fn();
    render(CharacterGrid, { characters, query: "", onselect });
    await fireEvent.click(screen.getByText("Acheron"));
    expect(onselect).toHaveBeenCalledWith(characters[0]);
  });
});