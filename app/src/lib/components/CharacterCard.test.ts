import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import CharacterCard from "./CharacterCard.svelte";
import type { CharacterSummary } from "$lib/api";

function c(enabled: number, total = 3): CharacterSummary {
  return { internal_name: "Firefly", display_name: "流萤", image: null, total, enabled };
}

describe("CharacterCard 信号灯", () => {
  it("恰好 1 个启用 = 绿灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(1), onclick: () => {} } });
    const dot = container.querySelector("span[title]")!;
    // jsdom 的 CSSStyleDeclaration 会把 #34c759 规范化为 rgb(52, 199, 89)
    expect(dot.getAttribute("style")).toContain("rgb(52, 199, 89)");
    expect(dot.getAttribute("title")).toBe("1 个 Mod 启用中");
  });
  it("2 个及以上 = 黄灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(2), onclick: () => {} } });
    expect(container.querySelector("span[title]")!.getAttribute("style")).toContain("rgb(255, 214, 10)");
  });
  it("0 个 = 灰灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(0, 0), onclick: () => {} } });
    const dot = container.querySelector("span[title]")!;
    expect(dot.getAttribute("style")).toContain("155, 155, 162");
    expect(dot.getAttribute("title")).toBe("没有启用的 Mod");
  });
});
