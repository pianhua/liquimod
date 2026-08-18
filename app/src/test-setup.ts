import "@testing-library/jest-dom/vitest";

type SimpleAnimation = {
  onfinish: (() => void) | null;
  effect: unknown;
  playState: string;
  currentTime: number;
  cancel: () => void;
  finish: () => void;
  pause: () => void;
  play: () => void;
  reverse: () => void;
  commitStyles: () => void;
  addEventListener: () => void;
  removeEventListener: () => void;
};

if (typeof Element !== "undefined" && !Element.prototype.animate) {
  Object.defineProperty(Element.prototype, "animate", {
    configurable: true,
    value(this: Element): SimpleAnimation {
      const animation: SimpleAnimation = {
        onfinish: null,
        effect: null,
        playState: "finished",
        currentTime: 0,
        cancel() {},
        finish() {},
        pause() {},
        play() {},
        reverse() {},
        commitStyles() {},
        addEventListener() {},
        removeEventListener() {},
      };
      queueMicrotask(() => {
        const cb = animation.onfinish;
        animation.onfinish = null;
        cb?.();
      });
      return animation;
    },
  });
}
