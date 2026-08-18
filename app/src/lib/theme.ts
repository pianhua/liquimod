export type ThemeChoice = "auto" | "light" | "dark";

export function resolveTheme(choice: string, systemDark: boolean): "light" | "dark" {
  if (choice === "dark") return "dark";
  if (choice === "light") return "light";
  return systemDark ? "dark" : "light";
}

let mediaHooked = false;

/// 按配置应用主题；auto 时跟随系统并监听系统切换（监听只挂一次）。
export function applyTheme(choice: string) {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  document.documentElement.dataset.theme = resolveTheme(choice, mq.matches);
  if (!mediaHooked) {
    mediaHooked = true;
    mq.addEventListener("change", () => {
      // 仅 auto 模式跟随；锁定亮/暗时忽略系统变化——读当前已解析值无法区分，
      // 因此在 +page 里保存当前 choice。此处重新读 dataset 上的 choice 标记。
      const c = document.documentElement.dataset.themeChoice ?? "auto";
      if (c === "auto") {
        document.documentElement.dataset.theme = mq.matches ? "dark" : "light";
      }
    });
  }
  document.documentElement.dataset.themeChoice =
    choice === "light" || choice === "dark" ? choice : "auto";
}