import {
  type MdxCompileOptions,
  type MdastPluginDefinition,
  type HastPluginDefinition,
  compileHandle,
  convertMdastToHastHandle,
  createMdastHandle,
  createMdxMdastHandle,
  dropHandle,
  renderHandle,
  serializeHandle,
  MdastReader,
  materializeMdastTree,
  HastReader,
  materializeHastTree,
  visitMdastHandle,
  resolveMdastSubscriptions,
  visitHastHandle,
  resolveHastSubscriptions,
  applyCommandsToMdastHandle,
  getHandleSource,
} from "satteri";
import { createHighlighterCore, type HighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import langJson from "shiki/langs/json.mjs";
import langTypescript from "shiki/langs/typescript.mjs";
import langMarkdown from "shiki/langs/markdown.mjs";
import langHtml from "shiki/langs/html.mjs";
import langJavascript from "shiki/langs/javascript.mjs";
import themeVitesseLight from "shiki/themes/vitesse-light.mjs";
import themeVitesseDark from "shiki/themes/vitesse-dark.mjs";

type Mode = "markdown" | "mdx";
type Tab = "mdast" | "hast" | "output" | "rendered";
type InputTab = "source" | "mdast-plugin" | "hast-plugin";

const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;

const input = $<HTMLTextAreaElement>("#input");
const inputMdastPlugin = $<HTMLTextAreaElement>("#input-mdast-plugin");
const inputHastPlugin = $<HTMLTextAreaElement>("#input-hast-plugin");
const highlightSource = $<HTMLPreElement>("#highlight-source");
const highlightMdastPlugin = $<HTMLPreElement>("#highlight-mdast-plugin");
const highlightHastPlugin = $<HTMLPreElement>("#highlight-hast-plugin");
const inputTabs = $<HTMLElement>("#input-tabs");
const outputTabs = $<HTMLElement>("#output-tabs");
const tabMdast = $<HTMLPreElement>("#tab-mdast");
const tabHast = $<HTMLPreElement>("#tab-hast");
const tabOutput = $<HTMLPreElement>("#tab-output");
const renderedFrame = $<HTMLIFrameElement>("#rendered-frame");
const loadingOverlay = $<HTMLElement>("#loading-overlay");
const optimizeToggle = $<HTMLInputElement>("#optimize-static-toggle");
const optimizeFields = $<HTMLElement>("#optimize-static-fields");
const optimizeFieldset = $<HTMLElement>("#optimize-static-fieldset");
const osComponent = $<HTMLInputElement>("#os-component");
const osProp = $<HTMLInputElement>("#os-prop");
const osWrapPropValue = $<HTMLInputElement>("#os-wrap-prop-value");
const osIgnoreElements = $<HTMLInputElement>("#os-ignore-elements");
const outputTabButton = $<HTMLButtonElement>('[data-tab="output"]');
const renderedTabButton = $<HTMLButtonElement>('[data-tab="rendered"]');
const statusBar = $<HTMLElement>("#status-bar");
const mdastPluginTab = $<HTMLButtonElement>('[data-input-tab="mdast-plugin"]');
const hastPluginTab = $<HTMLButtonElement>('[data-input-tab="hast-plugin"]');
const mdxOptionsFieldset = $<HTMLElement>("#mdx-options-fieldset");
const mdxJsxImportSource = $<HTMLInputElement>("#mdx-jsx-import-source");
const mdxJsxRuntime = $<HTMLSelectElement>("#mdx-jsx-runtime");
const mdxJsx = $<HTMLInputElement>("#mdx-jsx");
const mdxDevelopment = $<HTMLInputElement>("#mdx-development");
const mdxProviderImportSource = $<HTMLInputElement>("#mdx-provider-import-source");
const mdxOutputFormat = $<HTMLSelectElement>("#mdx-output-format");
const featGfm = $<HTMLInputElement>("#feat-gfm");
const featFrontmatter = $<HTMLInputElement>("#feat-frontmatter");
const featMath = $<HTMLInputElement>("#feat-math");
const featHeadingAttributes = $<HTMLInputElement>("#feat-heading-attributes");
const featDirective = $<HTMLInputElement>("#feat-directive");
const featSuperscript = $<HTMLInputElement>("#feat-superscript");
const featSubscript = $<HTMLInputElement>("#feat-subscript");
const featWikilinks = $<HTMLInputElement>("#feat-wikilinks");
const featSmartPunctuation = $<HTMLInputElement>("#feat-smart-punctuation");
const smartPunctOptions = $<HTMLFieldSetElement>("#smart-punct-options");
const featSmartQuotes = $<HTMLInputElement>("#feat-smart-quotes");
const featSmartDashes = $<HTMLInputElement>("#feat-smart-dashes");
const featSmartEllipses = $<HTMLInputElement>("#feat-smart-ellipses");

let currentMode: Mode = "markdown";
let activeTab: Tab = "mdast";
let compileGeneration = 0;
let highlighter: HighlighterCore | null = null;

let cachedMdastSource = "";
let cachedMdastPlugins: MdastPluginDefinition[] = [];
let cachedHastSource = "";
let cachedHastPlugins: HastPluginDefinition[] = [];

const currentShikiTheme = () =>
  document.documentElement.dataset.theme === "dark" ? "vitesse-dark" : "vitesse-light";

createHighlighterCore({
  themes: [themeVitesseLight, themeVitesseDark],
  langs: [langJson, langTypescript, langMarkdown, langHtml, langJavascript],
  engine: createJavaScriptRegexEngine(),
}).then((h) => {
  highlighter = h;
  highlightAllInputs();
});

function highlightInto(el: HTMLElement, code: string, lang: string) {
  if (!highlighter) {
    el.textContent = code;
    return;
  }
  const tokens = highlighter.codeToTokensBase(code, { lang, theme: currentShikiTheme() });
  let html = "";
  for (const line of tokens) {
    for (const token of line) {
      if (token.color) {
        html += `<span style="color:${token.color}">${escapeHtml(token.content)}</span>`;
      } else {
        html += escapeHtml(token.content);
      }
    }
    html += "\n";
  }
  el.innerHTML = html;
}

function highlightInput(textarea: HTMLTextAreaElement, pre: HTMLElement, lang: string) {
  highlightInto(pre, textarea.value, lang);
}

function highlightAllInputs() {
  highlightInput(input, highlightSource, "markdown");
  highlightInput(inputMdastPlugin, highlightMdastPlugin, "typescript");
  highlightInput(inputHastPlugin, highlightHastPlugin, "typescript");
}

let highlightTimer: ReturnType<typeof requestAnimationFrame> | null = null;
let pendingHighlights: { el: HTMLElement; code: string; lang: string }[] = [];

function scheduleOutputHighlights() {
  if (highlightTimer !== null) cancelAnimationFrame(highlightTimer);
  const work = pendingHighlights.slice();
  pendingHighlights = [];
  highlightTimer = requestAnimationFrame(() => {
    highlightTimer = null;
    const gen = compileGeneration;
    for (const { el, code, lang } of work) {
      if (compileGeneration !== gen) return;
      highlightInto(el, code, lang);
    }
  });
}

function syncScroll(textarea: HTMLTextAreaElement, pre: HTMLElement) {
  pre.scrollTop = textarea.scrollTop;
  pre.scrollLeft = textarea.scrollLeft;
}

function getMode(): Mode {
  return $<HTMLInputElement>('input[name="mode"]:checked').value as Mode;
}

function getFeatures() {
  return {
    gfm: featGfm.checked,
    frontmatter: featFrontmatter.checked,
    math: featMath.checked,
    headingAttributes: featHeadingAttributes.checked,
    directive: featDirective.checked,
    superscript: featSuperscript.checked,
    subscript: featSubscript.checked,
    wikilinks: featWikilinks.checked,
    smartPunctuation: featSmartPunctuation.checked,
    ...(featSmartPunctuation.checked &&
      !(featSmartQuotes.checked && featSmartDashes.checked && featSmartEllipses.checked) && {
        smartPunctuationOptions: {
          quotes: featSmartQuotes.checked,
          dashes: featSmartDashes.checked,
          ellipses: featSmartEllipses.checked,
        },
      }),
  };
}

function getMdxOptions() {
  if (currentMode !== "mdx") return undefined;
  const result: Record<string, any> = {};
  const jsxImportSource = mdxJsxImportSource.value.trim();
  if (jsxImportSource) result.jsxImportSource = jsxImportSource;
  const jsxRuntime = mdxJsxRuntime.value;
  if (jsxRuntime !== "automatic") result.jsxRuntime = jsxRuntime;
  if (mdxJsx.checked) result.jsx = true;
  if (mdxDevelopment.checked) result.development = true;
  const providerImportSource = mdxProviderImportSource.value.trim();
  if (providerImportSource) result.providerImportSource = providerImportSource;
  const outputFormat = mdxOutputFormat.value;
  if (outputFormat !== "program") result.outputFormat = outputFormat;

  const os = getOptimizeStatic();
  if (os) result.optimizeStatic = os;

  return Object.keys(result).length > 0 ? result : undefined;
}

function getOptimizeStatic(): MdxCompileOptions["optimizeStatic"] | undefined {
  if (currentMode !== "mdx" || !optimizeToggle.checked) return undefined;
  const ignoreRaw = osIgnoreElements.value.trim();
  // Spread the optional fields conditionally so they're omitted (not set to
  // undefined) when off — required by `exactOptionalPropertyTypes`.
  return {
    component: osComponent.value || "Fragment",
    prop: osProp.value || "set:html",
    ...(osWrapPropValue.checked && { wrapPropValue: true }),
    ...(ignoreRaw && { ignoreElements: ignoreRaw.split(",").map((s) => s.trim()) }),
  };
}

function updateModeUI() {
  currentMode = getMode();
  const isMdx = currentMode === "mdx";

  mdxOptionsFieldset.classList.toggle("hidden", !isMdx);
  optimizeFieldset.classList.toggle("hidden", !isMdx);
  outputTabButton.textContent = isMdx ? "JS" : "HTML";
  renderedTabButton.classList.toggle("hidden", isMdx);

  if (isMdx && activeTab === "rendered") {
    switchTab("output");
  }
}

function switchTab(tab: Tab) {
  activeTab = tab;
  document.querySelectorAll<HTMLElement>("#output-tabs .pg-tab").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.tab === tab);
  });
  document.querySelectorAll<HTMLElement>(".tab-pane").forEach((pane) => {
    pane.classList.toggle("active", pane.id === `tab-${tab}`);
  });
}

function switchInputTab(tab: InputTab) {
  document.querySelectorAll<HTMLElement>("#input-tabs .pg-tab").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.inputTab === tab);
  });
  document.querySelectorAll<HTMLElement>(".input-pane").forEach((pane) => {
    pane.classList.toggle("active", pane.dataset.inputPane === tab);
  });
}

function time<T>(fn: () => T): { result: T; ms: number } {
  const start = performance.now();
  const result = fn();
  return { result, ms: performance.now() - start };
}

function fmt(ms: number): string {
  return ms < 1 ? `${(ms * 1000).toFixed(0)}us` : `${ms.toFixed(1)}ms`;
}

async function evaluatePlugins<T extends { name: string }>(code: string): Promise<T[]> {
  const trimmed = code.trim();
  if (!trimmed) return [];

  const blob = new Blob([trimmed], { type: "text/javascript" });
  const url = URL.createObjectURL(blob);
  try {
    const mod = await import(/* @vite-ignore */ url);
    if (mod.default == null) {
      throw new Error("Plugin must use 'export default { ... }' or 'export default [...]'");
    }
    const raw = mod.default;
    const plugins = Array.isArray(raw) ? raw : [raw];
    for (const p of plugins) {
      if (!p.name) {
        throw new Error("Each plugin must have a 'name' property");
      }
    }
    return plugins as T[];
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function getMdastPlugins(): Promise<MdastPluginDefinition[]> {
  const source = inputMdastPlugin.value;
  if (source === cachedMdastSource) return cachedMdastPlugins;
  cachedMdastSource = source;
  cachedMdastPlugins = await evaluatePlugins<MdastPluginDefinition>(source);
  return cachedMdastPlugins;
}

async function getHastPlugins(): Promise<HastPluginDefinition[]> {
  const source = inputHastPlugin.value;
  if (source === cachedHastSource) return cachedHastPlugins;
  cachedHastSource = source;
  cachedHastPlugins = await evaluatePlugins<HastPluginDefinition>(source);
  return cachedHastPlugins;
}

async function compile() {
  const gen = ++compileGeneration;
  const source = input.value;
  const isMdx = currentMode === "mdx";
  const timings: string[] = [];
  let overhead = 0;

  let mdastPlugins: MdastPluginDefinition[] = [];
  let hastPlugins: HastPluginDefinition[] = [];
  try {
    mdastPlugins = await getMdastPlugins();
  } catch (e) {
    statusBar.innerHTML = `<span class="error">mdast plugin: ${escapeHtml(String(e))}</span>`;
    return;
  }
  try {
    hastPlugins = await getHastPlugins();
  } catch (e) {
    statusBar.innerHTML = `<span class="error">hast plugin: ${escapeHtml(String(e))}</span>`;
    return;
  }

  if (gen !== compileGeneration) return;

  const activeMdastCount = mdastPlugins.filter(
    (p) => resolveMdastSubscriptions(p).length > 0,
  ).length;
  const activeHastCount = hastPlugins.filter((p) => resolveHastSubscriptions(p).length > 0).length;
  updatePluginBadges(activeMdastCount, activeHastCount);

  const features = getFeatures();
  const totalStart = performance.now();
  try {
    const { result: mdastHandle, ms: parseMs } = time(() =>
      isMdx ? createMdxMdastHandle(source, features) : createMdastHandle(source, features),
    );
    timings.push(`parse → mdast <span>${fmt(parseMs)}</span>`);

    if (activeMdastCount > 0) {
      const pluginStart = performance.now();
      const handleSource = getHandleSource(mdastHandle);
      for (const plugin of mdastPlugins) {
        const subs = resolveMdastSubscriptions(plugin);
        const result = await visitMdastHandle(
          mdastHandle,
          plugin,
          subs,
          handleSource,
          "<playground>",
        );
        if (gen !== compileGeneration) return;
        if (result.hasMutations) {
          applyCommandsToMdastHandle(mdastHandle, result.commandBuffer);
        }
      }
      timings.push(`mdast plugins <span>${fmt(performance.now() - pluginStart)}</span>`);
    }

    const { result: mdastBuf, ms: mdastSerMs } = time(() => serializeHandle(mdastHandle));
    overhead += mdastSerMs;
    const { result: mdastTree, ms: mdastMatMs } = time(() =>
      materializeMdastTree(new MdastReader(mdastBuf)),
    );
    overhead += mdastMatMs;
    const mdastJson = JSON.stringify(mdastTree, null, 2);
    tabMdast.classList.remove("error");
    const { ms: mdastDomMs } = time(() => {
      tabMdast.textContent = mdastJson;
    });
    overhead += mdastDomMs;
    pendingHighlights.push({ el: tabMdast, code: mdastJson, lang: "json" });

    const { result: hastHandle, ms: convertMs } = time(() => convertMdastToHastHandle(mdastHandle));
    timings.push(`mdast → hast <span>${fmt(convertMs)}</span>`);

    if (activeHastCount > 0) {
      const pluginStart = performance.now();
      for (const plugin of hastPlugins) {
        const subs = resolveHastSubscriptions(plugin);
        await visitHastHandle(hastHandle, plugin, subs, source, "<playground>");
        if (gen !== compileGeneration) return;
      }
      timings.push(`hast plugins <span>${fmt(performance.now() - pluginStart)}</span>`);
    }

    const { result: hastBuf, ms: hastSerMs } = time(() => serializeHandle(hastHandle));
    overhead += hastSerMs;
    const { result: hastTree, ms: hastMatMs } = time(() =>
      materializeHastTree(new HastReader(hastBuf)),
    );
    overhead += hastMatMs;
    const hastJson = JSON.stringify(hastTree, null, 2);
    tabHast.classList.remove("error");
    const { ms: hastDomMs } = time(() => {
      tabHast.textContent = hastJson;
    });
    overhead += hastDomMs;
    pendingHighlights.push({ el: tabHast, code: hastJson, lang: "json" });

    let outputStr: string;
    if (isMdx) {
      const mdxOptions = getMdxOptions();
      const { result: js, ms } = time(() => compileHandle(hastHandle, mdxOptions));
      timings.push(`hast → js <span>${fmt(ms)}</span>`);
      outputStr = js;
    } else {
      const { result: html, ms } = time(() => renderHandle(hastHandle));
      timings.push(`hast → html <span>${fmt(ms)}</span>`);
      outputStr = html;
    }
    dropHandle(hastHandle);

    tabOutput.classList.remove("error");
    const outputLang: string = isMdx ? "javascript" : "html";
    const { ms: outputDomMs } = time(() => {
      tabOutput.textContent = outputStr;

      if (!isMdx) {
        const doc = renderedFrame.contentDocument;
        if (doc) {
          doc.open();
          doc.write(renderedFrameDocument(outputStr));
          doc.close();
        }
      }
    });
    overhead += outputDomMs;
    pendingHighlights.push({ el: tabOutput, code: outputStr, lang: outputLang });
  } catch (e) {
    const errStr = String(e);
    tabMdast.textContent = errStr;
    tabMdast.classList.add("error");
    tabHast.textContent = errStr;
    tabHast.classList.add("error");
    tabOutput.textContent = errStr;
    tabOutput.classList.add("error");
  }

  const totalMs = performance.now() - totalStart;
  const pipelineMs = totalMs - overhead;
  let totalHtml = `total <span>${fmt(pipelineMs)}</span>`;
  if (overhead > 0.01) {
    totalHtml += ` <span class="overhead" title="Includes ${fmt(overhead)} overhead from serializing ASTs, JSON stringifying, and DOM updates for the playground">(${fmt(totalMs)})</span>`;
  }
  timings.push(totalHtml);
  statusBar.innerHTML = timings.join(" · ");
  scheduleOutputHighlights();
}

function updatePluginBadges(mdastCount: number, hastCount: number) {
  mdastPluginTab.textContent = mdastCount > 0 ? `mdast plugin (${mdastCount})` : "mdast plugin";
  hastPluginTab.textContent = hastCount > 0 ? `hast plugin (${hastCount})` : "hast plugin";
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function scheduleCompile() {
  compile();
}

function renderedFrameDocument(body: string): string {
  const dark = document.documentElement.dataset.theme === "dark";
  // Inline both palettes so the iframe document is self-contained — it can't
  // reach out to the parent's CSS variables.
  const p = dark
    ? {
        bg: "#14120E",
        text: "#D8D0BE",
        ink: "#F2EAD3",
        surface: "#1E1B14",
        border: "#3A342A",
        secondary: "#B5AB95",
        codeBg: "#1E1B14",
        codeText: "#D8D0BE",
      }
    : {
        bg: "#F7F2E8",
        text: "#2A2620",
        ink: "#14110C",
        surface: "#EDE6D6",
        border: "#C9BFA8",
        secondary: "#5C5446",
        codeBg: "#14110C",
        codeText: "#F7F2E8",
      };
  return `<!doctype html>
<html>
<head><meta charset="utf-8"><style>
  body {
    font-family: "Iowan Old Style", "Palatino Linotype", Palatino, P052, serif;
    padding: 20px;
    line-height: 1.65;
    color: ${p.text};
    background: ${p.bg};
    max-width: 42rem;
    margin: 0 auto;
  }
  h1, h2, h3, h4, h5, h6 { color: ${p.ink}; letter-spacing: -0.01em; }
  pre {
    background: ${p.codeBg};
    color: ${p.codeText};
    padding: 0.9em 1em;
    border-radius: 3px;
    overflow-x: auto;
    font-size: 0.85em;
    line-height: 1.55;
    font-family: ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace;
  }
  code {
    background: ${p.surface};
    padding: 0.1em 0.35em;
    border-radius: 3px;
    font-size: 0.9em;
    font-family: ui-monospace, "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace;
  }
  pre code { background: none; padding: 0; color: inherit; }
  blockquote {
    border-left: 3px solid ${p.border};
    margin: 0;
    padding-left: 0.9em;
    color: ${p.secondary};
    font-style: italic;
  }
  img { max-width: 100%; }
  table { border-collapse: collapse; width: 100%; }
  th, td { border: 1px solid ${p.border}; padding: 0.4em 0.7em; text-align: left; }
  th { background: ${p.surface}; font-weight: 600; }
  a { color: ${p.secondary}; text-decoration: underline; text-underline-offset: 0.2em; }
</style></head>
<body>${body}</body>
</html>`;
}

// Re-render when the user toggles the site theme: re-highlight the editor
// textareas with the new shiki theme, and re-run the pipeline so the output
// panes and rendered iframe pick up the swap too.
new MutationObserver(() => {
  highlightAllInputs();
  scheduleCompile();
}).observe(document.documentElement, {
  attributes: true,
  attributeFilter: ["data-theme"],
});

inputTabs.addEventListener("click", (e) => {
  const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(".pg-tab");
  if (btn?.dataset.inputTab) {
    switchInputTab(btn.dataset.inputTab as InputTab);
  }
});

outputTabs.addEventListener("click", (e) => {
  const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(".pg-tab");
  if (btn?.dataset.tab) {
    switchTab(btn.dataset.tab as Tab);
  }
});

document.querySelectorAll('input[name="mode"]').forEach((el) => {
  el.addEventListener("change", () => {
    updateModeUI();
    highlightInput(input, highlightSource, "markdown");
    scheduleCompile();
  });
});

[
  featGfm,
  featFrontmatter,
  featMath,
  featHeadingAttributes,
  featDirective,
  featSuperscript,
  featSubscript,
  featWikilinks,
  featSmartPunctuation,
].forEach((el) => el.addEventListener("change", scheduleCompile));

featSmartPunctuation.addEventListener("change", () => {
  smartPunctOptions.classList.toggle("hidden", !featSmartPunctuation.checked);
});
[featSmartQuotes, featSmartDashes, featSmartEllipses].forEach((el) =>
  el.addEventListener("change", scheduleCompile),
);

[mdxJsxImportSource, mdxProviderImportSource].forEach((el) => {
  el.addEventListener("input", scheduleCompile);
});
[mdxJsxRuntime, mdxJsx, mdxDevelopment, mdxOutputFormat].forEach((el) => {
  el.addEventListener("change", scheduleCompile);
});

optimizeToggle.addEventListener("change", () => {
  optimizeFields.classList.toggle("hidden", !optimizeToggle.checked);
  scheduleCompile();
});

[osComponent, osProp, osWrapPropValue, osIgnoreElements].forEach((el) => {
  el.addEventListener("input", scheduleCompile);
  el.addEventListener("change", scheduleCompile);
});

const inputPairs: [HTMLTextAreaElement, HTMLElement, string][] = [
  [input, highlightSource, "markdown"],
  [inputMdastPlugin, highlightMdastPlugin, "typescript"],
  [inputHastPlugin, highlightHastPlugin, "typescript"],
];

for (const [textarea, pre, lang] of inputPairs) {
  textarea.addEventListener("input", () => {
    highlightInput(textarea, pre, lang);
    scheduleCompile();
  });
  textarea.addEventListener("scroll", () => syncScroll(textarea, pre));

  textarea.addEventListener("keydown", (e) => {
    if (e.key === "Tab") {
      e.preventDefault();
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      textarea.value = textarea.value.substring(0, start) + "  " + textarea.value.substring(end);
      textarea.selectionStart = textarea.selectionEnd = start + 2;
      highlightInput(textarea, pre, lang);
      scheduleCompile();
    }
  });
}

updateModeUI();

// The WASM module loads asynchronously (top-level await in wasi-browser.js).
// Reaching this line means the import chain resolved; hide the overlay.
loadingOverlay.classList.add("hidden");
highlightAllInputs();
compile();
