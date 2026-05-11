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
