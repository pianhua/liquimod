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

export const api = {
  getConfig: () => invoke<ConfigDto>("get_config"),
  chooseModsDir: (path: string) => invoke<ConfigDto>("choose_mods_dir", { path }),
  getCharacters: () => invoke<CharacterSummary[]>("get_characters"),
  listMods: (character: string) => invoke<ModDto[]>("list_mods", { character }),
  setModEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { id, enabled }),
};

/// 立绘 URL（vite publicDir 指向 assets/hsr）。
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