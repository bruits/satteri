// Lazy: the satteri + shiki imports are wrapped in dynamic `import()` so
// Rolldown splits them into their own chunks and we don't pay the WASM
// download / init cost (or Shiki's grammar) until the user is about to
// interact with the demo.

const installButton = document.querySelector<HTMLButtonElement>("#install-copy");
const installLabel = document.querySelector<HTMLSpanElement>("#install-copy-text");
if (installButton && installLabel) {
  let revertTimer: ReturnType<typeof setTimeout> | null = null;
  installButton.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText("pnpm add satteri");
      const original = installLabel.textContent;
      installLabel.textContent = "✓ copied";
      if (revertTimer !== null) clearTimeout(revertTimer);
      revertTimer = setTimeout(() => {
        installLabel.textContent = original;
        revertTimer = null;
      }, 1500);
    } catch {
      // Clipboard API can reject in insecure contexts; fail silently.
    }
  });
}

const SAMPLE = `# Markdown notes

Markdown was designed by John Gruber in 2004 as a plain-text format that
converts to HTML. It has since splintered into dozens of dialects.

## Common dialects

- **CommonMark** is the strict spec most parsers target today
- **GFM** adds tables, task lists, and strikethrough
- [**MDX**](https://mdxjs.com) lets you embed JSX inside Markdown documents

> "The single biggest source of inspiration for Markdown's syntax is the
> format of plain text email." (John Gruber, 2004)

\`\`\`ts
import { markdownToHtml } from "satteri";

const { html } = markdownToHtml("# Hello, *world*");
\`\`\`

| Dialect    | Tables | Math |
| ---------- | ------ | ---- |
| CommonMark | no     | no   |
| GFM        | yes    | no   |
| MDX        | yes    | opt  |
`;

const input = document.querySelector<HTMLTextAreaElement>("#demo-input");
const output = document.querySelector<HTMLDivElement>("#demo-output");
const stat = document.querySelector<HTMLSpanElement>("#demo-stat");
const status = document.querySelector<HTMLDivElement>("#demo-status");
const highlight = document.querySelector<HTMLPreElement>("#demo-highlight");
const highlightCode = highlight?.querySelector<HTMLElement>("code") ?? null;
const docsPerSec = document.querySelector<HTMLSpanElement>("#demo-docs-per-sec");

if (input && output && stat && status && highlight && highlightCode) {
  input.value = SAMPLE;
  // Render plaintext immediately so the editor isn't blank during idle wait.
  highlightCode.textContent = SAMPLE;
  output.innerHTML = `<p class="text-tertiary italic">Loading…</p>`;

  const fmt = (ms: number) => (ms < 1 ? `${(ms * 1000).toFixed(0)}μs` : `${ms.toFixed(2)}ms`);

  type Compile = (source: string) => string;
  type Highlighter = (source: string) => string;
  let compile: Compile | null = null;
  let highlightHtml: Highlighter | null = null;
  let pending: number | null = null;
  let started = false;
  let loadingPromise: Promise<void> | null = null;

  function escapeHtml(s: string): string {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function applyHighlight() {
    if (!highlightHtml || !input || !highlightCode) return;
    highlightCode.innerHTML = highlightHtml(input.value);
  }

  function syncScroll() {
    if (!highlight || !input) return;
    highlight.scrollTop = input.scrollTop;
    highlight.scrollLeft = input.scrollLeft;
  }

  function schedule() {
    if (pending !== null) cancelAnimationFrame(pending);
    pending = requestAnimationFrame(() => {
      pending = null;
      run();
    });
  }

  function run() {
    if (!compile || !input || !output || !stat) return;
    const source = input.value;
    const start = performance.now();
    try {
      const html = compile(source);
      const ms = performance.now() - start;
      output.innerHTML = html;
      stat.textContent = fmt(ms);
      status?.classList.remove("text-red-700");
    } catch (e) {
      output.innerHTML = `<pre class="text-red-700 whitespace-pre-wrap">${escapeHtml(String(e))}</pre>`;
      stat.textContent = "error";
      status?.classList.add("text-red-700");
    }
  }

  function measureThroughput(markdownToHtml: (s: string) => { html: string }) {
    if (!docsPerSec) return;
    // Run as many compiles of the sample as we can in ~80ms. This is short
    // enough to feel instant on slow machines, long enough to amortize
    // performance.now() noise. The number we display is conservative: actual
    // batch throughput in Node with the native binding is meaningfully
    // higher (no V8 boundary crossings).
    const budgetMs = 80;
    const start = performance.now();
    let count = 0;
    while (performance.now() - start < budgetMs) {
      markdownToHtml(SAMPLE);
      count++;
    }
    const elapsed = performance.now() - start;
    const perSec = (count / elapsed) * 1000;
    // Floor to two significant figures so the displayed number reads as a
    // round value (14,723 → 14,000) and stays an honest underclaim.
    const mag = Math.pow(10, Math.max(0, Math.floor(Math.log10(perSec)) - 1));
    const rounded = Math.floor(perSec / mag) * mag;
    docsPerSec.textContent = rounded.toLocaleString();
  }

  async function start_demo() {
    if (started) return loadingPromise!;
    started = true;
    if (!stat || !output) return;

    stat.textContent = "loading wasm…";

    loadingPromise = (async () => {
      const [satteri, shikiCore, jsEngine, langMarkdown, themeLight, themeDark] = await Promise.all(
        [
          import("satteri"),
          import("shiki/core"),
          import("shiki/engine/javascript"),
          import("shiki/langs/markdown.mjs"),
          import("shiki/themes/vitesse-light.mjs"),
          import("shiki/themes/vitesse-dark.mjs"),
        ],
      );

      const highlighter = await shikiCore.createHighlighterCore({
        themes: [themeLight.default, themeDark.default],
        langs: [langMarkdown.default],
        engine: jsEngine.createJavaScriptRegexEngine(),
      });

      const themeFor = () =>
        document.documentElement.dataset.theme === "dark" ? "vitesse-dark" : "vitesse-light";

      highlightHtml = (source: string) => {
        const tokens = highlighter.codeToTokensBase(source, {
          lang: "markdown",
          theme: themeFor(),
        });
        let out = "";
        for (const line of tokens) {
          for (const token of line) {
            if (token.color) {
              out += `<span style="color:${token.color}">${escapeHtml(token.content)}</span>`;
            } else {
              out += escapeHtml(token.content);
            }
          }
          out += "\n";
        }
        return out;
      };

      // Re-highlight when the user toggles the site theme so token colors
      // track the active palette instead of staying frozen at first paint.
      new MutationObserver(() => applyHighlight()).observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme"],
      });

      // Warm up V8's WASM tier-up so the first user-visible compile shows
      // steady-state timing (~µs) instead of the Liftoff-baseline number.
      for (let i = 0; i < 3; i++) satteri.markdownToHtml(SAMPLE);
      compile = (source: string) => satteri.markdownToHtml(source).html;

      applyHighlight();
      run();
      // Throughput micro-benchmark: tight loop on the sample for a fixed time
      // budget. Runs once after warmup so visitors see a real per-machine
      // number instead of a marketing claim. Don't await it — it shouldn't
      // block the first compile from displaying.
      requestAnimationFrame(() => measureThroughput(satteri.markdownToHtml));
    })();

    return loadingPromise;
  }

  input.addEventListener("input", () => {
    applyHighlight();
    if (!started) {
      void start_demo().then(schedule);
    } else {
      schedule();
    }
  });
  input.addEventListener("scroll", syncScroll);

  // Kick off the WASM download + first compile during browser idle time so it
  // doesn't compete with initial page paint. `requestIdleCallback` is ideal;
  // fall back to a `setTimeout` for browsers that don't ship it (Safari).
  const kickoff = () => void start_demo();
  if ("requestIdleCallback" in window) {
    (window as Window & typeof globalThis).requestIdleCallback(kickoff, { timeout: 3000 });
  } else {
    setTimeout(kickoff, 500);
  }
}
