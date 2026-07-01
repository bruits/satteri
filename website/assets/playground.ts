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
import satteriPkg from "../../packages/satteri/package.json" with { type: "json" };

const SATTERI_VERSION = satteriPkg.version;

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
const alertBar = $<HTMLElement>("#alert-bar");
const alertBarMessage = $<HTMLElement>("#alert-bar-message");
const alertBarRunScripts = $<HTMLButtonElement>("#alert-bar-run-scripts");
const alertBarResetScripts = $<HTMLButtonElement>("#alert-bar-reset-scripts");
const alertBarDismiss = $<HTMLButtonElement>("#alert-bar-dismiss");
const statusBar = $<HTMLElement>("#status-bar");
const shareButton = $<HTMLButtonElement>("#pg-share");
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

// --- Shareable links -------------------------------------------------------
// The full editor state is deflate-compressed and base64url-encoded into the
// URL hash, so a link is self-contained and never touches the static server.

type FeatureKey =
  | "gfm"
  | "frontmatter"
  | "math"
  | "headingAttributes"
  | "directive"
  | "superscript"
  | "subscript"
  | "wikilinks"
  | "smartPunctuation"
  | "smartQuotes"
  | "smartDashes"
  | "smartEllipses";

interface MdxState {
  jsxImportSource: string;
  jsxRuntime: string;
  jsx: boolean;
  development: boolean;
  providerImportSource: string;
  outputFormat: string;
}

interface OptimizeStaticState {
  enabled: boolean;
  component: string;
  prop: string;
  wrapPropValue: boolean;
  ignoreElements: string;
}

interface PlaygroundState {
  version: string;
  mode?: Mode;
  source?: string;
  mdastPlugin?: string;
  hastPlugin?: string;
  features?: Partial<Record<FeatureKey, boolean>>;
  mdx?: Partial<MdxState>;
  optimizeStatic?: Partial<OptimizeStaticState>;
}

function getState(): PlaygroundState {
  return {
    version: SATTERI_VERSION,
    mode: getMode(),
    source: input.value,
    mdastPlugin: inputMdastPlugin.value,
    hastPlugin: inputHastPlugin.value,
    features: {
      gfm: featGfm.checked,
      frontmatter: featFrontmatter.checked,
      math: featMath.checked,
      headingAttributes: featHeadingAttributes.checked,
      directive: featDirective.checked,
      superscript: featSuperscript.checked,
      subscript: featSubscript.checked,
      wikilinks: featWikilinks.checked,
      smartPunctuation: featSmartPunctuation.checked,
      smartQuotes: featSmartQuotes.checked,
      smartDashes: featSmartDashes.checked,
      smartEllipses: featSmartEllipses.checked,
    },
    mdx: {
      jsxImportSource: mdxJsxImportSource.value,
      jsxRuntime: mdxJsxRuntime.value,
      jsx: mdxJsx.checked,
      development: mdxDevelopment.checked,
      providerImportSource: mdxProviderImportSource.value,
      outputFormat: mdxOutputFormat.value,
    },
    optimizeStatic: {
      enabled: optimizeToggle.checked,
      component: osComponent.value,
      prop: osProp.value,
      wrapPropValue: osWrapPropValue.checked,
      ignoreElements: osIgnoreElements.value,
    },
  };
}

function applyState(state: PlaygroundState) {
  const mode: Mode = state.mode === "mdx" ? "mdx" : "markdown";
  $<HTMLInputElement>(`input[name="mode"][value="${mode}"]`).checked = true;
  input.value = state.source ?? "";
  inputMdastPlugin.value = state.mdastPlugin ?? "";
  inputHastPlugin.value = state.hastPlugin ?? "";

  const features = state.features ?? {};
  featGfm.checked = !!features.gfm;
  featFrontmatter.checked = !!features.frontmatter;
  featMath.checked = !!features.math;
  featHeadingAttributes.checked = !!features.headingAttributes;
  featDirective.checked = !!features.directive;
  featSuperscript.checked = !!features.superscript;
  featSubscript.checked = !!features.subscript;
  featWikilinks.checked = !!features.wikilinks;
  featSmartPunctuation.checked = !!features.smartPunctuation;
  featSmartQuotes.checked = !!features.smartQuotes;
  featSmartDashes.checked = !!features.smartDashes;
  featSmartEllipses.checked = !!features.smartEllipses;
  smartPunctOptions.classList.toggle("hidden", !featSmartPunctuation.checked);

  const mdx = state.mdx ?? {};
  mdxJsxImportSource.value = mdx.jsxImportSource ?? "";
  mdxJsxRuntime.value = mdx.jsxRuntime ?? "automatic";
  mdxJsx.checked = !!mdx.jsx;
  mdxDevelopment.checked = !!mdx.development;
  mdxProviderImportSource.value = mdx.providerImportSource ?? "";
  mdxOutputFormat.value = mdx.outputFormat ?? "program";

  const os = state.optimizeStatic ?? {};
  optimizeToggle.checked = !!os.enabled;
  osComponent.value = os.component ?? "Fragment";
  osProp.value = os.prop ?? "set:html";
  osWrapPropValue.checked = !!os.wrapPropValue;
  osIgnoreElements.value = os.ignoreElements ?? "";
  optimizeFields.classList.toggle("hidden", !optimizeToggle.checked);

  updateModeUI();
}

async function compress(text: string): Promise<Uint8Array> {
  const stream = new CompressionStream("deflate-raw");
  // Drain the readable side first so a large payload can't deadlock on backpressure.
  const read = new Response(stream.readable).arrayBuffer();
  const writer = stream.writable.getWriter();
  await writer.write(new TextEncoder().encode(text));
  await writer.close();
  return new Uint8Array(await read);
}

async function decompress(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const stream = new DecompressionStream("deflate-raw");
  const read = new Response(stream.readable).arrayBuffer();
  const writer = stream.writable.getWriter();
  await writer.write(bytes);
  await writer.close();
  return new TextDecoder().decode(await read);
}

function bytesToBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBytes(value: string): Uint8Array<ArrayBuffer> {
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/");
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

const SHARE_HASH_PREFIX = "#s=";

async function buildShareUrl(): Promise<string> {
  const payload = bytesToBase64Url(await compress(JSON.stringify(getState())));
  return `${location.origin}${location.pathname}${SHARE_HASH_PREFIX}${payload}`;
}

async function loadSharedState(): Promise<PlaygroundState | null> {
  if (!location.hash.startsWith(SHARE_HASH_PREFIX)) return null;
  try {
    const json = await decompress(base64UrlToBytes(location.hash.slice(SHARE_HASH_PREFIX.length)));
    return JSON.parse(json) as PlaygroundState;
  } catch {
    showAlert(
      "Could not restore the shared playground state. Using the default playground instead.",
      { dismissible: true },
    );
    return null;
  }
}

const defaultMdastPluginSource = inputMdastPlugin.value;
const defaultHastPluginSource = inputHastPlugin.value;
let hasPendingScripts = false;
let pendingVersionWarning: string | undefined;

function sharedStateHasUnsafePlugins(state: PlaygroundState): boolean {
  const mdastPlugin = state.mdastPlugin ?? "";
  const hastPlugin = state.hastPlugin ?? "";

  return (
    (mdastPlugin.trim() !== "" && mdastPlugin !== defaultMdastPluginSource) ||
    (hastPlugin.trim() !== "" && hastPlugin !== defaultHastPluginSource)
  );
}

async function applySharedState(): Promise<boolean> {
  if (!location.hash.startsWith(SHARE_HASH_PREFIX)) return false;

  const shared = await loadSharedState();

  const url = new URL(location.href);
  url.hash = "";
  history.replaceState(null, "", url);

  if (!shared) return false;

  pendingVersionWarning = shared.version !== SATTERI_VERSION ? shared.version : undefined;

  hasPendingScripts = sharedStateHasUnsafePlugins(shared);
  if (hasPendingScripts) compileGeneration++;

  applyState(shared);
  highlightAllInputs();

  if (hasPendingScripts) {
    showUnsafePluginAlert();
  } else {
    showVersionWarning();
    compile();
  }

  return true;
}

function runPendingPlugins() {
  hasPendingScripts = false;
  showVersionWarning();
  compile();
}

function resetPendingPlugins() {
  hasPendingScripts = false;
  dismissAlert();

  inputMdastPlugin.value = defaultMdastPluginSource;
  inputHastPlugin.value = defaultHastPluginSource;
  highlightInput(inputMdastPlugin, highlightMdastPlugin, "typescript");
  highlightInput(inputHastPlugin, highlightHastPlugin, "typescript");

  showVersionWarning();
  compile();
}

// TODO(HiDeoo) still needed
// TODO(HiDeoo) better flash
let shareResetTimer: ReturnType<typeof setTimeout> | null = null;
function flashShareLabel(label: string) {
  shareButton.textContent = label;
  if (shareResetTimer !== null) clearTimeout(shareResetTimer);
  shareResetTimer = setTimeout(() => {
    shareButton.textContent = "Share";
    shareResetTimer = null;
  }, 1500);
}

async function shareCurrentState() {
  if (!hasPendingScripts) dismissAlert();
  let url: string;
  try {
    url = await buildShareUrl();
  } catch {
    showAlert("Could not create a share link with the current playground state.");
    return;
  }
  try {
    await navigator.clipboard.writeText(url);
    flashShareLabel("Copied!");
  } catch {
    showAlert(
      `Could not copy the <a href="${escapeHtml(url)}" rel="noopener noreferrer">share link</a> to the clipboard. You can still copy it manually.`,
      { html: true },
    );
  }
}

shareButton.addEventListener("click", () => void shareCurrentState());
alertBarRunScripts.addEventListener("click", runPendingPlugins);
alertBarResetScripts.addEventListener("click", resetPendingPlugins);
alertBarDismiss.addEventListener("click", dismissAlert);

function showAlert(message: string, opts?: { html?: boolean; dismissible?: boolean }) {
  if (hasPendingScripts) return;

  if (opts?.html) {
    alertBarMessage.innerHTML = message;
  } else {
    alertBarMessage.textContent = message;
  }
  alertBarRunScripts.hidden = true;
  alertBarResetScripts.hidden = true;
  alertBarDismiss.hidden = opts?.dismissible !== true;
  alertBar.hidden = false;
}

function showUnsafePluginAlert() {
  statusBar.innerHTML = `<span class="error">Review plugin scripts before running the playground.</span>`;

  alertBarMessage.textContent =
    "The shared playground link includes plugin scripts. Please review them before running the playground.";

  alertBarDismiss.hidden = true;
  alertBarRunScripts.hidden = false;
  alertBarResetScripts.hidden = false;
  alertBar.hidden = false;
}

function showVersionWarning() {
  if (!pendingVersionWarning) {
    dismissAlert();
    return;
  }

  showAlert(
    `This shared playground link was created with Sätteri ${pendingVersionWarning}, but this playground uses
 Sätteri ${SATTERI_VERSION}. Some options and behavior may differ.`,
    { dismissible: true },
  );

  pendingVersionWarning = undefined;
}

function dismissAlert() {
  alertBar.hidden = true;
  alertBarMessage.textContent = "";
  alertBarRunScripts.hidden = true;
  alertBarResetScripts.hidden = true;
  alertBarDismiss.hidden = true;
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
  if (hasPendingScripts) {
    compileGeneration++;
    return;
  }

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

  if (gen !== compileGeneration || hasPendingScripts) return;

  try {
    hastPlugins = await getHastPlugins();
  } catch (e) {
    statusBar.innerHTML = `<span class="error">hast plugin: ${escapeHtml(String(e))}</span>`;
    return;
  }

  if (gen !== compileGeneration || hasPendingScripts) return;

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
          undefined,
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
    const { result: mdastJson, ms: mdastJsonMs } = time(() => JSON.stringify(mdastTree, null, 2));
    overhead += mdastJsonMs;
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
        await visitHastHandle(hastHandle, plugin, subs, source, undefined);
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
    const { result: hastJson, ms: hastJsonMs } = time(() => JSON.stringify(hastTree, null, 2));
    overhead += hastJsonMs;
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
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
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

// Mobile-only options toggle; on desktop `hidden md:flex` keeps the fieldsets open.
const pgSidebarToggle = document.getElementById("pg-sidebar-toggle");
const pgSidebarContent = document.getElementById("pg-sidebar-content");
const pgSidebarChevron = document.getElementById("pg-sidebar-chevron");
pgSidebarToggle?.addEventListener("click", () => {
  if (!pgSidebarContent) return;
  const open = pgSidebarContent.classList.toggle("hidden") === false;
  pgSidebarToggle.setAttribute("aria-expanded", open ? "true" : "false");
  pgSidebarChevron?.classList.toggle("rotate-180", open);
});

// The WASM module loads asynchronously (top-level await in wasi-browser.js).
// Reaching this line means the import chain resolved; hide the overlay.
loadingOverlay.classList.add("hidden");

window.addEventListener("hashchange", () => {
  void applySharedState();
});

void applySharedState().then((applied) => {
  if (!applied) {
    highlightAllInputs();
    compile();
  }
});
