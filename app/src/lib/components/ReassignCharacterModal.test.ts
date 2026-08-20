import { render, fireEvent, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ReassignCharacterModal from "./ReassignCharacterModal.svelte";
import type { CharacterSummary, ModDto } from "$lib/api";

const mod: ModDto = {
  id: 1,
  name: "Kafka Dress",
  enabled: false,
  installed_at: 1000,
  thumb: null,
  size_bytes: 1024,
  file_count: 10,
  path: "C:/mock/KafkaDress",
  category_id: null,
  note: null,
  cover_image: null,
};

const characters: CharacterSummary[] = [
  {
    internal_name: "Kafka",
    display_name: "卡芙卡",
    total: 3,
    enabled: 1,
    image: "Kafka.png",
  },
  {
    internal_name: "Acheron",
    display_name: "黄泉",
    total: 5,
    enabled: 2,
    image: "Acheron.png",
  },
  {
    internal_name: "Others",
    display_name: "其他",
    total: 2,
    enabled: 0,
    image: null,
  },
];

describe("ReassignCharacterModal", () => {
  it("正确渲染标题和角色列表", () => {
    render(ReassignCharacterModal, {
      props: {
        mod,
        currentCharacter: "Others",
        characters,
        onClose: vi.fn(),
        onReassigned: vi.fn(),
      },
    });

    screen.getByText("重新分配角色");
    screen.getByText("卡芙卡");
    screen.getByText("黄泉");
  });

  it("支持搜索过滤角色", async () => {
    render(ReassignCharacterModal, {
      props: {
        mod,
        currentCharacter: "Others",
        characters,
        onClose: vi.fn(),
        onReassigned: vi.fn(),
      },
    });

    const input = screen.getByPlaceholderText(/搜索角色/);
    await fireEvent.input(input, { target: { value: "黄泉" } });

    expect(screen.queryByText("卡芙卡")).toBeNull();
    screen.getByText("黄泉");
  });

  it("当输入不存在的角色名时显示新建提示", async () => {
    render(ReassignCharacterModal, {
      props: {
        mod,
        currentCharacter: "Others",
        characters,
        onClose: vi.fn(),
        onReassigned: vi.fn(),
      },
    });

    const input = screen.getByPlaceholderText(/搜索角色/);
    await fireEvent.input(input, { target: { value: "Castorice" } });

    screen.getByText("新建角色「Castorice」");
  });
});
