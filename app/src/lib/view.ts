import type { ModDto } from "$lib/api";

export type View =
  | { kind: "home" }
  | { kind: "all" }
  | { kind: "uncat" }
  | { kind: "category"; id: number; name: string }
  | { kind: "character"; name: string; display: string };

export type ModSort = "recent" | "name" | "enabled";

export function viewKey(v: View): string {
  switch (v.kind) {
    case "home":
      return "home";
    case "all":
      return "all";
    case "uncat":
      return "uncat";
    case "category":
      return `cat:${v.id}`;
    case "character":
      return `char:${v.name}`;
  }
}

export function filterMods(mods: ModDto[], query: string): ModDto[] {
  const q = query.trim().toLowerCase();
  if (!q) return mods;
  return mods.filter((m) => m.name.toLowerCase().includes(q));
}

export function sortMods(mods: ModDto[], sort: ModSort): ModDto[] {
  const arr = [...mods];
  switch (sort) {
    case "recent":
      return arr.sort((a, b) => b.installed_at - a.installed_at);
    case "name":
      return arr.sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
    case "enabled":
      return arr.sort(
        (a, b) => Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "zh-Hans-CN"),
      );
  }
}
