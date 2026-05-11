// Pairs with the inline THEME_INIT in layout.rs that sets data-theme on
// first paint. This module just handles the toggle click + persistence.
const root = document.documentElement;
const button = document.querySelector<HTMLButtonElement>("#theme-toggle");

function apply(theme: "light" | "dark") {
  root.dataset.theme = theme;
  try {
    localStorage.setItem("theme", theme);
  } catch {}
}

button?.addEventListener("click", () => {
  apply(root.dataset.theme === "dark" ? "light" : "dark");
});

// Follow the OS preference until the user makes an explicit choice.
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
  if (localStorage.getItem("theme")) return;
  apply(e.matches ? "dark" : "light");
});
