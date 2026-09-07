const fmt = (ms: number) => (ms < 1 ? `${(ms * 1000).toFixed(0)}μs` : `${ms.toFixed(2)}ms`);

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

const themeFor = () =>
  document.documentElement.dataset.theme === "dark" ? "vitesse-dark" : "vitesse-light";

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

  type Compile = (source: string) => string;
  type Highlighter = (source: string) => string;
  let compile: Compile | null = null;
  let highlightHtml: Highlighter | null = null;
  let pending: number | null = null;
  let started = false;
  let loadingPromise: Promise<void> | null = null;

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
    // An 80ms sample amortizes timer noise without delaying interaction noticeably.
    const budgetMs = 80;
    const start = performance.now();
    let count = 0;
    while (performance.now() - start < budgetMs) {
      markdownToHtml(SAMPLE);
      count++;
    }
    const elapsed = performance.now() - start;
    const perSec = (count / elapsed) * 1000;
    // Round down so the displayed throughput does not overstate the measurement.
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
      // Dynamic imports defer WASM and Shiki downloads until the demo is near interaction.
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

      new MutationObserver(() => applyHighlight()).observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-theme"],
      });

      // Warm up WASM compilation tiers before displaying a steady-state timing.
      for (let i = 0; i < 3; i++) satteri.markdownToHtml(SAMPLE);
      compile = (source: string) => satteri.markdownToHtml(source).html;

      applyHighlight();
      run();
      // Do not await throughput measurement; the first compile should display immediately.
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

  // Defer WASM initialization until idle so it does not compete with the initial page paint.
  const kickoff = () => void start_demo();
  if ("requestIdleCallback" in window) {
    (window as Window & typeof globalThis).requestIdleCallback(kickoff, { timeout: 3000 });
  } else {
    setTimeout(kickoff, 500);
  }
}
