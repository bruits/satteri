use maud::{Markup, PreEscaped, html};
use maudit::route::prelude::*;

use crate::layout::{LayoutOptions, SeoMeta, layout_with_options};

const SAMPLE_MARKDOWN: &str = "# Hello World

This is a **Markdown** playground for Sätteri.

- Item 1
- Item 2
- Item 3

```js
console.log(\"hello\");
```";

const SAMPLE_MDAST_PLUGIN: &str = "export default [
  {
    name: \"my-plugin\",
    // heading(node, ctx) {
    //   ...
    // },
  },
]";

const SAMPLE_HAST_PLUGIN: &str = "export default [
  {
    name: \"my-plugin\",
    // element: {
    //   filter: [\"h1\"],
    //   visit(node, ctx) {
    //     ...
    //   },
    // },
  },
]";

#[route("/playground/")]
pub struct Playground;

impl Route for Playground {
    fn render(&self, ctx: &mut PageContext) -> impl Into<RenderResult> {
        ctx.assets.include_script("assets/playground.ts")?;
        layout_with_options(
            playground_body(),
            ctx,
            Some(SeoMeta {
                title: "Playground".to_string(),
                description:
                    "Edit Markdown or MDX, watch the mdast, hast, and HTML output update live."
                        .to_string(),
            }),
            LayoutOptions { fullscreen: true },
        )
    }
}

fn playground_body() -> Markup {
    html! {
        div .flex.flex-col."min-h-0"."md:h-full" {
            div #alert-bar.pg-alert-bar hidden {
                span #alert-bar-message.pg-alert-message role="status" aria-live="polite" {}
                button #alert-bar-reset-scripts.pg-alert-action type="button" hidden {
                    "Reset scripts"
                }
                button #alert-bar-run-scripts.pg-alert-action type="button" hidden {
                    "Run playground"
                }
                button #alert-bar-dismiss.pg-alert-dismiss type="button" aria-label="Dismiss alert" hidden {
                    svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" aria-hidden="true" {
                        path fill="currentColor" d="M15.71 8.29a1 1 0 0 0-1.42 0L12 10.59l-2.29-2.3a1 1 0 0 0-1.42 1.42l2.3 2.29-2.3 2.29a1 1 0 0 0 0 1.42 1 1 0 0 0 1.42 0l2.29-2.3 2.29 2.3a1 1 0 0 0 1.42 0 1 1 0 0 0 0-1.42L13.41 12l2.3-2.29a1 1 0 0 0 0-1.42m3.36-3.36A10 10 0 1 0 4.93 19.07 10 10 0 1 0 19.07 4.93m-1.41 12.73A8 8 0 1 1 20 12a7.95 7.95 0 0 1-2.34 5.66" {}
                    }
                }
            }
            div #playground.flex.flex-col."md:grid"."md:grid-cols-[16rem_1fr_1fr]"."flex-1".min-h-0 {
                (sidebar())
                (editor_panel())
                (output_panel())
            }
            (loading_overlay())
        }
    }
}

fn sidebar() -> Markup {
    html! {
        aside #pg-sidebar.bg-surface."md:border-r".border-b."md:border-b-0".border-border."md:overflow-y-auto".flex.flex-col {
            button #pg-sidebar-toggle
                type="button"
                aria-controls="pg-sidebar-content"
                aria-expanded="false"
                class="md:hidden flex items-center justify-between px-4 py-3 text-xs uppercase tracking-widest text-tertiary font-bold cursor-pointer" {
                span { "Playground options" }
                svg #pg-sidebar-chevron width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" class="transition-transform" {
                    path d="M6 9l6 6 6-6" {}
                }
            }
            div #pg-sidebar-content .hidden.flex."md:flex".flex-col.gap-4.p-4 {
                (mode_fieldset())
                (features_fieldset())
                (mdx_options_fieldset())
                (optimize_static_fieldset())
                (share_button())
            }
        }
    }
}

fn mode_fieldset() -> Markup {
    html! {
        fieldset.pg-fieldset {
            legend.pg-legend { "Mode" }
            label.pg-label {
                input type="radio" name="mode" value="markdown" checked;
                "Markdown"
            }
            label.pg-label {
                input type="radio" name="mode" value="mdx";
                "MDX"
            }
        }
    }
}

fn features_fieldset() -> Markup {
    html! {
        fieldset #features-fieldset .pg-fieldset {
            legend.pg-legend { "Features" }
            label.pg-label { input type="checkbox" #feat-gfm checked; " GFM" }
            label.pg-label { input type="checkbox" #feat-frontmatter checked; " Frontmatter" }
            label.pg-label { input type="checkbox" #feat-math checked; " Math" }
            label.pg-label { input type="checkbox" #feat-heading-attributes checked; " Heading attributes" }
            label.pg-label { input type="checkbox" #feat-directive; " Directive" }
            label.pg-label { input type="checkbox" #feat-superscript; " Superscript" }
            label.pg-label { input type="checkbox" #feat-subscript; " Subscript" }
            label.pg-label { input type="checkbox" #feat-wikilinks; " Wikilinks" }
            label.pg-label { input type="checkbox" #feat-smart-punctuation; " Smart punctuation" }
            fieldset #smart-punct-options .pg-subfieldset.hidden {
                label.pg-label { input type="checkbox" #feat-smart-quotes checked; " Quotes" }
                label.pg-label { input type="checkbox" #feat-smart-dashes checked; " Dashes" }
                label.pg-label { input type="checkbox" #feat-smart-ellipses checked; " Ellipses" }
            }
        }
    }
}

fn mdx_options_fieldset() -> Markup {
    html! {
        fieldset #mdx-options-fieldset .pg-fieldset.hidden {
            legend.pg-legend { "MDX Options" }
            label.pg-label-stack {
                "jsxImportSource"
                input type="text" #mdx-jsx-import-source value="" placeholder="react (default)" .pg-input;
            }
            label.pg-label-stack {
                "jsxRuntime"
                select #mdx-jsx-runtime .pg-input {
                    option value="automatic" selected { "automatic" }
                    option value="classic" { "classic" }
                }
            }
            label.pg-label {
                input type="checkbox" #mdx-jsx;
                " jsx (keep JSX)"
            }
            label.pg-label {
                input type="checkbox" #mdx-development;
                " development"
            }
            label.pg-label-stack {
                "providerImportSource"
                input type="text" #mdx-provider-import-source value="" placeholder="none" .pg-input;
            }
            label.pg-label-stack {
                "outputFormat"
                select #mdx-output-format .pg-input {
                    option value="program" selected { "program" }
                    option value="function-body" { "function-body" }
                }
            }
        }
    }
}

fn optimize_static_fieldset() -> Markup {
    html! {
        fieldset #optimize-static-fieldset .pg-fieldset.hidden {
            legend.pg-legend { "optimizeStatic" }
            label.pg-label {
                input type="checkbox" #optimize-static-toggle;
                " Enable"
            }
            div #optimize-static-fields .hidden.flex.flex-col.gap-2.mt-2 {
                label.pg-label-stack {
                    "component"
                    input type="text" #os-component value="Fragment" .pg-input;
                }
                label.pg-label-stack {
                    "prop"
                    input type="text" #os-prop value="set:html" .pg-input;
                }
                label.pg-label {
                    input type="checkbox" #os-wrap-prop-value;
                    " wrapPropValue"
                }
                label.pg-label-stack {
                    "ignoreElements"
                    input type="text" #os-ignore-elements placeholder="comma-separated" .pg-input;
                }
            }
        }
    }
}

fn share_button() -> Markup {
    html! {
        button #pg-share
            type="button"
            title="Copy a shareable link to your clipboard with the current playground content"
            aria-label="Copy share link"
            class="pg-input pg-input-button pt-3.5 pb-2.5" {
            "Share"
        }
        span #pg-share-status .sr-only role="status" {}
    }
}

fn editor_panel() -> Markup {
    html! {
        section #editor-panel.flex.flex-col."md:border-r".border-b."md:border-b-0".border-border."min-w-0"."min-h-[60vh]"."md:min-h-0" {
            nav #input-tabs .pg-tabbar {
                button.pg-tab.active data-input-tab="source" { "Source" }
                button.pg-tab data-input-tab="mdast-plugin" { "mdast plugin" }
                button.pg-tab data-input-tab="hast-plugin" { "hast plugin" }
            }
            div #input-content .relative.flex-1.min-h-0.overflow-hidden {
                div.input-pane.active data-input-pane="source" {
                    pre #highlight-source .pg-input-highlight { code {} }
                    textarea #input .pg-input-textarea spellcheck="false" {
                        (SAMPLE_MARKDOWN)
                    }
                }
                div.input-pane data-input-pane="mdast-plugin" {
                    pre #highlight-mdast-plugin .pg-input-highlight { code {} }
                    textarea #input-mdast-plugin .pg-input-textarea spellcheck="false" {
                        (SAMPLE_MDAST_PLUGIN)
                    }
                }
                div.input-pane data-input-pane="hast-plugin" {
                    pre #highlight-hast-plugin .pg-input-highlight { code {} }
                    textarea #input-hast-plugin .pg-input-textarea spellcheck="false" {
                        (SAMPLE_HAST_PLUGIN)
                    }
                }
            }
        }
    }
}

fn output_panel() -> Markup {
    html! {
        section #output-panel.flex.flex-col.overflow-hidden."min-w-0"."min-h-[60vh]"."md:min-h-0" {
            nav #output-tabs .pg-tabbar {
                button.pg-tab.active data-tab="mdast" { "mdast" }
                button.pg-tab data-tab="hast" { "hast" }
                button.pg-tab data-tab="output" { "HTML" }
                button.pg-tab data-tab="rendered" { "Rendered" }
            }
            div #output-content .relative.flex-1.min-h-0.overflow-hidden {
                pre #tab-mdast .tab-pane.active.pg-output-pane {}
                pre #tab-hast .tab-pane.pg-output-pane {}
                pre #tab-output .tab-pane.pg-output-pane {}
                div #tab-rendered .tab-pane.pg-rendered-pane {
                    iframe #rendered-frame sandbox="allow-same-origin" {}
                }
            }
            div #status-bar .pg-status-bar {}
        }
    }
}

fn loading_overlay() -> Markup {
    html! {
        div #loading-overlay .pg-loading-overlay {
            (PreEscaped("<p>Loading WASM&hellip;</p>"))
        }
    }
}
