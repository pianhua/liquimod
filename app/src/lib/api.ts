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
  thumb: string | null;
}

export interface PresetDto {
  id: number;
  name: string;
  created_at: number;
}

export interface ApplyResultDto {
  enabled: number;
  disabled: number;
}

export type InstallResult =
  | { status: "installed"; mod_id: number; name: string; character: string; warnings: string[] }
  | { status: "needs_password" };

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
  { id: 1, name: "Summer Skin", enabled: true, installed_at: 1755000000, thumb: null },
  { id: 2, name: "Battle FX+", enabled: false, installed_at: 1755100000, thumb: null },
  { id: 3, name: "HD Textures", enabled: false, installed_at: 1755200000, thumb: null },
];

const mockPresets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1755000000 },
  { id: 2, name: "截图模式", created_at: 1755100000 },
];

const mockPasswords: string[] = ["1234"];

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    switch (cmd) {
      case "get_config":
        return { library_root: "C:/mock/Library", mods_dir: null } as T;
      case "get_characters":
        return structuredClone(mockCharacters) as T;
      case "list_mods":
        return structuredClone(mockMods) as T;
      case "install_mod": {
        await new Promise((r) => setTimeout(r, 800));
        const p = String(args?.path ?? "");
        if (p.includes("locked") && args?.password == null)
          return { status: "needs_password" } as T;
        if (p.includes("locked") && args?.password !== "1234")
          return { status: "needs_password" } as T;
        return {
          status: "installed",
          mod_id: 99,
          name: p.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "Mod",
          character: "Firefly",
          warnings: [],
        } as T;
      }
      case "uninstall_mod":
        return undefined as T;
      case "list_presets":
        return structuredClone(mockPresets) as T;
      case "save_preset": {
        const p = { id: mockPresets.length + 1, name: String(args?.name ?? "预设"), created_at: 1755000000 };
        const i = mockPresets.findIndex((x) => x.name === p.name);
        if (i >= 0) mockPresets[i] = { ...p, id: mockPresets[i].id };
        else mockPresets.push(p);
        return structuredClone(p) as T;
      }
      case "apply_preset":
        return { enabled: 2, disabled: 1 } as T;
      case "delete_preset": {
        const i = mockPresets.findIndex((x) => x.id === Number(args?.id));
        if (i >= 0) mockPresets.splice(i, 1);
        return undefined as T;
      }
      case "list_passwords":
        return structuredClone(mockPasswords) as T;
      case "add_password":
        mockPasswords.push(String(args?.value ?? ""));
        return undefined as T;
      case "remove_password": {
        const i = mockPasswords.indexOf(String(args?.value));
        if (i >= 0) mockPasswords.splice(i, 1);
        return undefined as T;
      }
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
  installMod: (path: string, character?: string | null, password?: string | null) =>
    call<InstallResult>("install_mod", { path, character: character ?? null, password: password ?? null }),
  uninstallMod: (id: number) => call<void>("uninstall_mod", { id }),
  listPresets: () => call<PresetDto[]>("list_presets"),
  savePreset: (name: string) => call<PresetDto>("save_preset", { name }),
  applyPreset: (id: number, name: string) =>
    call<ApplyResultDto>("apply_preset", { id, name }),
  deletePreset: (id: number) => call<void>("delete_preset", { id }),
  listPasswords: () => call<string[]>("list_passwords"),
  addPassword: (value: string) => call<void>("add_password", { value }),
  removePassword: (value: string) => call<void>("remove_password", { value }),
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
