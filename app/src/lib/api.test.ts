import { describe, expect, it } from "vitest";
import { filterCharacters, portraitUrl, type CharacterSummary } from "./api";

const list: CharacterSummary[] = [
  { internal_name: "Acheron", display_name: "Acheron", image: "acheron.png", total: 2, enabled: 1 },
  { internal_name: "Firefly", display_name: "Firefly", image: "firefly.png", total: 0, enabled: 0 },
];

describe("filterCharacters", () => {
  it("空查询返回全部", () => {
    expect(filterCharacters(list, "  ")).toHaveLength(2);
  });
  it("按显示名或内部名过滤（不区分大小写）", () => {
    expect(filterCharacters(list, "fire").map((c) => c.internal_name)).toEqual(["Firefly"]);
    expect(filterCharacters(list, "ACHERON")).toHaveLength(1);
  });
  it("无匹配返回空", () => {
    expect(filterCharacters(list, "zzz")).toEqual([]);
  });
});

describe("portraitUrl", () => {
  it("拼接 images 路径", () => {
    expect(portraitUrl("acheron.png")).toBe("/images/acheron.png");
  });
});