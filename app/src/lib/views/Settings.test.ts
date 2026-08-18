import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import Settings from "./Settings.svelte";
import { api } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPasswords: vi.fn(),
      addPassword: vi.fn(),
      removePassword: vi.fn(),
    },
    isTauri: () => false,
  };
});

const config = { library_root: "C:/mock/Library", mods_dir: "D:/game/Mods" };

describe("Settings", () => {
  beforeEach(() => {
    vi.mocked(api.listPasswords).mockResolvedValue(["1234"]);
    vi.mocked(api.addPassword).mockResolvedValue(undefined);
    vi.mocked(api.removePassword).mockResolvedValue(undefined);
  });

  it("显示目录配置", () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    expect(screen.getByText("C:/mock/Library")).toBeTruthy();
    expect(screen.getByText("D:/game/Mods")).toBeTruthy();
  });

  it("加载并展示密码本", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => expect(screen.getByText("1234")).toBeTruthy());
  });

  it("添加密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await fireEvent.input(screen.getByPlaceholderText("添加解压密码…"), {
      target: { value: "abc" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "添加" }));
    expect(api.addPassword).toHaveBeenCalledWith("abc");
  });

  it("移除密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => screen.getByText("1234"));
    await fireEvent.click(screen.getByLabelText("移除密码 1234"));
    expect(api.removePassword).toHaveBeenCalledWith("1234");
  });

  it("返回回调", async () => {
    const onback = vi.fn();
    render(Settings, { props: { config, onback, onchanged: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: /返回/ }));
    expect(onback).toHaveBeenCalled();
  });
});
