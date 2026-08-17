import { invoke } from "@tauri-apps/api/core";

export interface ConfigDto {
  library_root: string;
  mods_dir: string | null;
}

export interface CharacterSummary {
  internal_name: string;
  display_name: string;
  image: string | null;
  total: number;
  enabled: number;
}

export interface ModDto {
  id: number;
  name: string;
  enabled: boolean;
  installed_at: number;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const mockCharacters: CharacterSummary[] = [
  ["Acheron", "acheron.png", 3, 1],
  ["Firefly", "firefly.png", 2, 2],
  ["Castorice", "castorice.png", 1, 0],
  ["Cipher", "cipher.png", 0, 0],
  ["Aglaea", "aglaea.png", 5, 3],
  ["Anaxa", "anaxa.png", 0, 0],
  ["Archer", "archer.png", 2, 1],
  ["Argenti", "argenti.png", 0, 0],
  ["Arlan", "arlan.png", 0, 0],
  ["Asta", "asta.png", 1, 1],
  ["Aventurine", "aventurine.png", 0, 0],
  ["Bailu", "bailu.png", 0, 0],
  ["Others", null, 2, 0],
].map(([name, image, total, enabled]) => ({
  internal_name: name as string,
  display_name: name === "Others" ? "其他" : (name as string),
  image: image as string | null,
  total: total as number,
  enabled: enabled as number,
}));

const mockMods: ModDto[] = [
  { id: 1, name: "Summer Skin", enabled: true, installed_at: 1755000000 },
  { id: 2, name: "Battle FX+", enabled: false, installed_at: 1755100000 },
  { id: 3, name: "HD Textures", enabled: false, installed_at: 1755200000 },
];

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    switch (cmd) {
      case "get_config":
        return { library_root: "C:/mock/Library", mods_dir: null } as T;
      case "get_characters":
        return structuredClone(mockCharacters) as T;
      case "list_mods":
        return structuredClone(mockMods) as T;
      default:
        return undefined as T;
    }
  }
  return invoke<T>(cmd, args);
}

export const api = {
  getConfig: () => call<ConfigDto>("get_config"),
  chooseModsDir: (path: string) => call<ConfigDto>("choose_mods_dir", { path }),
  getCharacters: () => call<CharacterSummary[]>("get_characters"),
  listMods: (character: string) => call<ModDto[]>("list_mods", { character }),
  setModEnabled: (id: number, enabled: boolean) =>
    call<void>("set_mod_enabled", { id, enabled }),
};

/// 立绘 URL（SvelteKit files.assets 指向 assets/hsr）。
export function portraitUrl(image: string): string {
  return `/images/${image}`;
}

/// 网格搜索过滤（不区分大小写，匹配显示名与内部名）。
export function filterCharacters(
  list: CharacterSummary[],
  query: string,
): CharacterSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (c) =>
      c.display_name.toLowerCase().includes(q) ||
      c.internal_name.toLowerCase().includes(q),
  );
}
