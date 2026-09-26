const button = document.getElementById("mobile-menu-button");
const panel = document.getElementById("mobile-menu-panel");
const iconOpen = document.getElementById("mobile-menu-icon-open");
const iconClose = document.getElementById("mobile-menu-icon-close");

let open = false;

function setOpen(next: boolean) {
  if (!panel) return;
  open = next;
  panel.classList.toggle("opacity-0", !open);
  panel.classList.toggle("-translate-y-2", !open);
  panel.classList.toggle("pointer-events-none", !open);
  iconOpen?.classList.toggle("hidden", open);
  iconClose?.classList.toggle("hidden", !open);
  button?.setAttribute("aria-expanded", open ? "true" : "false");
  document.body.style.overflow = open ? "hidden" : "";
}

button?.addEventListener("click", () => setOpen(!open));

// Close the mobile panel when the viewport reaches desktop width.
matchMedia("(min-width: 768px)").addEventListener("change", (e) => {
  if (e.matches && open) setOpen(false);
});
