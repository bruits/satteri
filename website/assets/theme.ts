// Pairs with the inline THEME_INIT in layout.rs that sets data-theme on
// first paint. This module just handles the toggle click + persistence.
// Multiple `.theme-toggle` buttons can exist (one per responsive nav).
const root = document.documentElement;
const buttons = document.querySelectorAll<HTMLButtonElement>(".theme-toggle");

function apply(theme: "light" | "dark") {
  root.dataset.theme = theme;
  root.dataset.pfTheme = theme;
  try {
    localStorage.setItem("theme", theme);
  } catch {}
}

buttons.forEach((button) => {
  button.addEventListener("click", () => {
    apply(root.dataset.theme === "dark" ? "light" : "dark");
  });
});

// Follow the OS preference until the user makes an explicit choice.
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
  if (localStorage.getItem("theme")) return;
  apply(e.matches ? "dark" : "light");
});
