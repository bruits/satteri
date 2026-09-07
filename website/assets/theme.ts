// Initial theme selection runs inline in layout.rs to avoid a flash before this module loads.
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

matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => {
  if (localStorage.getItem("theme")) return;
  apply(e.matches ? "dark" : "light");
});
