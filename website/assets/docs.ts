// "Use this page" dropdown: copy markdown to clipboard or open it in
// Claude/ChatGPT with a pre-filled prompt. The raw markdown lives in a
// <template> the route renders alongside the dropdown.
const button = document.querySelector<HTMLButtonElement>("#page-actions-button");
const menu = document.querySelector<HTMLDivElement>("#page-actions-menu");
const sourceTemplate = document.querySelector<HTMLTemplateElement>("#page-source");

if (button && menu && sourceTemplate) {
  const markdown = sourceTemplate.content.textContent ?? "";

  function open() {
    if (!menu || !button) return;
    menu.classList.remove("hidden");
    button.setAttribute("aria-expanded", "true");
  }

  function close() {
    if (!menu || !button) return;
    menu.classList.add("hidden");
    button.setAttribute("aria-expanded", "false");
  }

  button.addEventListener("click", (e) => {
    e.stopPropagation();
    if (menu.classList.contains("hidden")) open();
    else close();
  });

  document.addEventListener("click", (e) => {
    const target = e.target as Node;
    if (!menu.contains(target) && !button.contains(target)) close();
  });

  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") close();
  });

  function buildPrompt(): string {
    return `Help me understand this Sätteri docs page: ${window.location.href}`;
  }

  document
    .querySelector<HTMLButtonElement>("#action-copy-md")
    ?.addEventListener("click", async (e) => {
      const btn = e.currentTarget as HTMLButtonElement;
      try {
        await navigator.clipboard.writeText(markdown);
        const original = btn.textContent;
        btn.textContent = "✓ Copied";
        setTimeout(() => {
          btn.textContent = original;
          close();
        }, 900);
      } catch {
        close();
      }
    });

  document.querySelector<HTMLButtonElement>("#action-claude")?.addEventListener("click", () => {
    window.open(
      `https://claude.ai/new?q=${encodeURIComponent(buildPrompt())}`,
      "_blank",
      "noopener",
    );
    close();
  });

  document.querySelector<HTMLButtonElement>("#action-chatgpt")?.addEventListener("click", () => {
    window.open(
      `https://chatgpt.com/?q=${encodeURIComponent(buildPrompt())}`,
      "_blank",
      "noopener",
    );
    close();
  });
}

// Package-manager tabs (emitted by the `install` shortcode). Clicking a tab
// switches every install snippet on the page at once and persists the choice
// so the reader's preferred manager sticks across pages.
const PM_KEY = "pkgManager";
const DEFAULT_PM = "pnpm";

function setActivePm(pm: string) {
  document.querySelectorAll<HTMLElement>("[data-pkg-tabs]").forEach((group) => {
    group.querySelectorAll<HTMLElement>(".pkg-tab").forEach((btn) => {
      btn.classList.toggle("active", btn.dataset.pkg === pm);
    });
    group.querySelectorAll<HTMLElement>(".pkg-tab-panel").forEach((panel) => {
      panel.classList.toggle("active", panel.dataset.pkg === pm);
    });
  });
}

let initialPm = DEFAULT_PM;
try {
  initialPm = localStorage.getItem(PM_KEY) || DEFAULT_PM;
} catch {}
if (initialPm !== DEFAULT_PM) setActivePm(initialPm);

document.querySelectorAll<HTMLElement>("[data-pkg-tabs]").forEach((group) => {
  group.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLElement>(".pkg-tab");
    if (!btn) return;
    const pm = btn.dataset.pkg;
    if (!pm) return;
    setActivePm(pm);
    try {
      localStorage.setItem(PM_KEY, pm);
    } catch {}
  });
});
