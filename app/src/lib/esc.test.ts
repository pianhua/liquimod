import { describe, expect, it } from "vitest";
import { pushEscHandler, dispatchEscape } from "./esc";

describe("esc hierarchy stack", () => {
  it("按栈顶优先（LIFO）消费 Esc", () => {
    let order: string[] = [];
    const popBase = pushEscHandler(() => {
      order.push("base");
      return true;
    });

    const popModal = pushEscHandler(() => {
      order.push("modal");
      return true;
    });

    const popLightbox = pushEscHandler(() => {
      order.push("lightbox");
      return true;
    });

    // 第一次按 Esc：应该只触发 lightbox
    expect(dispatchEscape()).toBe(true);
    expect(order).toEqual(["lightbox"]);
    popLightbox();

    // 第二次按 Esc：应该触发 modal
    order = [];
    expect(dispatchEscape()).toBe(true);
    expect(order).toEqual(["modal"]);
    popModal();

    // 第三次按 Esc：应该触发 base
    order = [];
    expect(dispatchEscape()).toBe(true);
    expect(order).toEqual(["base"]);
    popBase();

    // 栈空时应返回 false
    expect(dispatchEscape()).toBe(false);
  });
});
