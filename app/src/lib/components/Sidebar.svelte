<script lang="ts">
  import { api, type CategoryDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import type { View } from "$lib/view";
  import SearchBar from "./SearchBar.svelte";
  import PresetMenu from "./PresetMenu.svelte";

  let {
    view,
    categories,
    charCatName,
    charCount,
    query = $bindable(),
    onnavigate,
    onchanged,
    onapplied,
  }: {
    view: View;
    categories: CategoryDto[];
    charCatName: string;
    charCount: number;
    query: string;
    onnavigate: (v: View) => void;
    onchanged: () => void;
    onapplied: () => void;
  } = $props();

  function isActive(key: string): boolean {
    if (key === "home") return view.kind === "home" || view.kind === "character";
    return view.kind === "type" && String(view.id) === key;
  }

  // 侧边栏只展示固定分类（kind 非空），按 ord 排序；角色是虚拟大类在最上。
  let fixedTypes = $derived(
    categories
      .filter((c) => c.kind != null)
      .sort((a, b) => a.ord - b.ord),
  );
</script>

<aside class="w-52 shrink-0 flex flex-col min-h-0 px-3 pb-3 pt-1">
  <div class="pb-2.5 shrink-0">
    <SearchBar bind:value={query} />
  </div>
  <nav class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-0.5" aria-label="分类导航">
    <button
      class="flex items-center justify-between h-9 px-3 radius-card text-sm cursor-pointer transition-colors"
      style={isActive("home")
        ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
        : ""}
      aria-current={isActive("home") ? "page" : undefined}
      onclick={() => onnavigate({ kind: "home" })}
    >
      <span class="truncate">{charCatName}</span>
      <span class="text-xs text-secondary shrink-0">{charCount}</span>
    </button>

    {#if fixedTypes.length > 0}
      <div class="mx-3 my-2 shrink-0" style="border-top: 0.5px solid var(--glass-stroke)"></div>
    {/if}

    {#each fixedTypes as c (c.id)}
      <button
        class="w-full flex items-center justify-between h-9 px-3 radius-card text-sm cursor-pointer transition-colors"
        style={isActive(String(c.id))
          ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
          : ""}
        aria-current={isActive(String(c.id)) ? "page" : undefined}
        onclick={() => onnavigate({ kind: "type", id: c.id, name: c.name })}
      >
        <span class="truncate">{c.name}</span>
        <span class="text-xs text-secondary shrink-0">{c.mod_count}</span>
      </button>
    {/each}
  </nav>

  <div class="shrink-0 pt-2">
    <div class="pb-1.5">
      <PresetMenu {onapplied} block />
    </div>
  </div>
</aside>