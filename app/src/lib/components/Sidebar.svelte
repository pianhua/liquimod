<script lang="ts">
  import { api, type CategoryDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import type { View } from "$lib/view";
  import SearchBar from "./SearchBar.svelte";

  let {
    view,
    categories,
    charCatName,
    allCount,
    charCount,
    uncatCount,
    query = $bindable(),
    onnavigate,
    onchanged,
  }: {
    view: View;
    categories: CategoryDto[];
    charCatName: string;
    allCount: number;
    charCount: number;
    uncatCount: number;
    query: string;
    onnavigate: (v: View) => void;
    onchanged: () => void;
  } = $props();

  let creating = $state(false);
  let newName = $state("");
  let renamingId = $state<number | null>(null);
  let renameDraft = $state("");
  let renameCancelled = $state(false);
  let menuFor = $state<number | null>(null);
  let confirmingDelete = $state<number | null>(null);
  let busy = $state(false);

  function isActive(key: string): boolean {
    if (key === "all") return view.kind === "all";
    if (key === "home") return view.kind === "home" || view.kind === "character";
    if (key === "uncat") return view.kind === "uncat";
    return view.kind === "category" && String(view.id) === key;
  }

  async function createCategory() {
    const v = newName.trim();
    if (!v || busy) {
      creating = false;
      return;
    }
    busy = true;
    try {
      await api.createCategory(v);
      newName = "";
      creating = false;
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  function startRename(c: CategoryDto) {
    menuFor = null;
    renamingId = c.id;
    renameDraft = c.name;
  }

  async function commitRename(id: number) {
    if (renameCancelled) {
      renameCancelled = false;
      return;
    }
    const v = renameDraft.trim();
    renamingId = null;
    if (!v || busy) return;
    busy = true;
    try {
      await api.renameCategory(id, v);
      // 重命名当前正在查看的分类时同步面包屑
      if (view.kind === "category" && view.id === id) {
        onnavigate({ kind: "category", id, name: v });
      } else {
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function move(id: number, delta: number) {
    if (busy) return;
    busy = true;
    menuFor = null;
    try {
      await api.moveCategory(id, delta);
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function remove(c: CategoryDto) {
    if (confirmingDelete !== c.id) {
      confirmingDelete = c.id;
      return;
    }
    if (busy) return;
    busy = true;
    menuFor = null;
    confirmingDelete = null;
    try {
      await api.deleteCategory(c.id);
      if (view.kind === "category" && view.id === c.id) onnavigate({ kind: "home" });
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      menuFor = null;
      confirmingDelete = null;
    }
  }}
/>

<aside class="w-52 shrink-0 flex flex-col min-h-0 px-3 pb-3 pt-1">
  <div class="pb-2.5 shrink-0">
    <SearchBar bind:value={query} />
  </div>
  <nav class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-0.5" aria-label="分类导航">
    {#each [
      { key: "all", label: "全部 Mod", count: allCount },
      { key: "home", label: charCatName, count: charCount },
      { key: "uncat", label: "未分类", count: uncatCount },
    ] as item (item.key)}
      <button
        class="flex items-center justify-between h-9 px-3 radius-card text-sm cursor-pointer transition-colors"
        style={isActive(item.key)
          ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
          : ""}
        aria-current={isActive(item.key) ? "page" : undefined}
        onclick={() =>
          onnavigate(item.key === "all" ? { kind: "all" } : item.key === "home" ? { kind: "home" } : { kind: "uncat" })}
      >
        <span class="truncate">{item.label}</span>
        <span class="text-xs text-secondary shrink-0">{item.count}</span>
      </button>
    {/each}

    {#if categories.length > 0}
      <div class="mx-3 my-2 shrink-0" style="border-top: 0.5px solid var(--glass-stroke)"></div>
    {/if}

    {#each categories as c (c.id)}
      <div class="relative">
        {#if renamingId === c.id}
          <input
            bind:value={renameDraft}
            aria-label={`重命名分类 ${c.name}`}
            class="w-full h-9 px-3 text-sm bg-transparent outline-none radius-card"
            style="box-shadow: inset 0 0 0 1.5px var(--accent)"
            onkeydown={(e) => {
              if (e.key === "Enter") commitRename(c.id);
              else if (e.key === "Escape") {
                renameCancelled = true;
                renamingId = null;
              }
            }}
            onblur={() => commitRename(c.id)}
            autofocus
          />
        {:else}
          <button
            class="w-full flex items-center justify-between h-9 pl-3 pr-1.5 radius-card text-sm cursor-pointer transition-colors"
            style={isActive(String(c.id))
              ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
              : ""}
            aria-current={isActive(String(c.id)) ? "page" : undefined}
            onclick={() => onnavigate({ kind: "category", id: c.id, name: c.name })}
          >
            <span class="truncate">{c.name}</span>
            <span class="flex items-center gap-0.5 shrink-0">
              <span class="text-xs text-secondary">{c.mod_count}</span>
              <span
                role="button"
                tabindex="0"
                aria-label={`分类操作 ${c.name}`}
                class="w-6 h-6 grid place-items-center rounded-full text-secondary transition-colors hover:bg-[var(--glass-stroke)]"
                onclick={(e) => {
                  e.stopPropagation();
                  confirmingDelete = null;
                  menuFor = menuFor === c.id ? null : c.id;
                }}
                onkeydown={(e) => {
                  if (e.key === "Enter") {
                    e.stopPropagation();
                    menuFor = menuFor === c.id ? null : c.id;
                  }
                }}
              >
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                  <circle cx="6" cy="2.5" r="1.2" /><circle cx="6" cy="6" r="1.2" /><circle cx="6" cy="9.5" r="1.2" />
                </svg>
              </span>
            </span>
          </button>
        {/if}
        {#if menuFor === c.id}
          <button
            class="fixed inset-0 z-40 cursor-default bg-transparent"
            aria-label="关闭分类菜单"
            tabindex="-1"
            onclick={() => {
              menuFor = null;
              confirmingDelete = null;
            }}
          ></button>
          <div class="glass radius-panel absolute right-0 top-10 z-50 w-44 p-1.5 flex flex-col gap-0.5">
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)]" onclick={() => startRename(c)}>重命名</button>
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] disabled:opacity-40" disabled={busy} onclick={() => move(c.id, -1)}>上移</button>
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] disabled:opacity-40" disabled={busy} onclick={() => move(c.id, 1)}>下移</button>
            <button
              class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors"
              style={confirmingDelete === c.id ? "background: var(--danger); color: white" : "color: var(--danger)"}
              onclick={() => remove(c)}
            >
              {confirmingDelete === c.id
                ? c.mod_count > 0
                  ? `确认删除（${c.mod_count} 个 Mod 移回）`
                  : "确认删除"
                : "删除"}
            </button>
          </div>
        {/if}
      </div>
    {/each}
  </nav>

  <div class="shrink-0 pt-2">
    {#if creating}
      <input
        bind:value={newName}
        aria-label="新分类名称"
        placeholder="分类名称…"
        class="w-full h-9 px-3 text-sm bg-transparent outline-none radius-card"
        style="box-shadow: inset 0 0 0 1.5px var(--accent)"
        onkeydown={(e) => {
          if (e.key === "Enter") createCategory();
          else if (e.key === "Escape") {
            newName = "";
            creating = false;
          }
        }}
        onblur={createCategory}
        autofocus
      />
    {:else}
      <button
        class="w-full h-9 px-3 radius-card text-sm text-secondary cursor-pointer text-left transition-colors hover:bg-[var(--glass-stroke)]"
        onclick={() => (creating = true)}
      >
        ＋ 新建分类
      </button>
    {/if}
  </div>
</aside>
