const toggle = document.getElementById("docs-sidebar-toggle");
const close = document.getElementById("docs-sidebar-close");
const overlay = document.getElementById("docs-mobile-sidebar");
const panel = document.getElementById("docs-mobile-sidebar-panel");

let open = false;

function setOpen(next: boolean) {
  if (!overlay || !panel) return;
  open = next;
  overlay.classList.toggle("opacity-0", !open);
  overlay.classList.toggle("opacity-100", open);
  overlay.classList.toggle("pointer-events-none", !open);
  panel.classList.toggle("-translate-x-full", !open);
  panel.classList.toggle("translate-x-0", open);
  toggle?.setAttribute("aria-expanded", open ? "true" : "false");
  document.body.style.overflow = open ? "hidden" : "";
}

toggle?.addEventListener("click", () => setOpen(true));
close?.addEventListener("click", () => setOpen(false));

overlay?.addEventListener("click", (event) => {
  if (event.target === overlay) setOpen(false);
});

// Close before navigation so the next page cannot inherit an open overlay.
panel?.addEventListener("click", (event) => {
  const target = event.target as HTMLElement | null;
  if (target?.closest("a")) setOpen(false);
});

matchMedia("(min-width: 768px)").addEventListener("change", (e) => {
  if (e.matches && open) setOpen(false);
});
