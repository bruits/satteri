use maud::{Markup, PreEscaped, html};
use maudit::route::prelude::*;

use crate::layout::{SeoMeta, layout};

const FLOURISH: &str = include_str!("../../assets/flourish.svg");

#[route("/")]
pub struct Index;

impl Route for Index {
    fn render(&self, ctx: &mut PageContext) -> impl Into<RenderResult> {
        ctx.assets.include_script("assets/demo.ts")?;
        layout(
            html! {
                (hero())
                (demo())
            },
            ctx,
            Some(SeoMeta::default()),
        )
    }
}

fn hero() -> Markup {
    html! {
        section.border-b.border-border {
            div.max-w-5xl.mx-auto.px-6.pt-10.sm:pt-20.pb-16.sm:pb-24.text-center {
                div.text-tertiary.mb-6.flex.justify-center."[&_svg]:h-5"."[&_svg]:w-auto" {
                    (PreEscaped(FLOURISH))
                }
                h1.font-logo.text-6xl.sm:text-7xl.leading-tight.mb-6 {
                    "A Markdown pipeline "
                    span.italic.text-secondary { "forged in Rust" }
                    " for the "
                    span.italic.text-secondary { "JavaScript world" }
                    "."
                }
                p.text-lg.text-secondary.max-w-4xl.mx-auto.mb-8 {
                    "Sätteri puts flexible JavaScript plugins on top of a fast Rust Markdown / MDX engine."
                    br;
                    "Best of both worlds."
                }
                button #install-copy
                    type="button"
                    aria-label="Copy install command"
                    title="Copy"
                    class="inline-block mb-8 px-4 py-2 bg-surface text-secondary font-mono text-sm rounded-sm cursor-pointer hover:text-ink transition-colors" {
                    span #install-copy-text { "$ pnpm add satteri" }
                }
                div.flex.gap-4.justify-center.flex-wrap.items-center {
                    a.no-underline.bg-ink.text-paper.inline-flex.items-center.justify-center."px-6"."pt-3.5"."pb-2.5".leading-none.font-medium.rounded-sm.hover:opacity-90 href="/docs/quick-start/" {
                        "Get started"
                    }
                    a.no-underline.border.border-border.inline-flex.items-center.justify-center."px-6"."pt-3.5"."pb-2.5".leading-none.font-medium.rounded-sm.text-ink.hover:bg-surface href="https://github.com/bruits/satteri" {
                        "View on GitHub"
                    }
                }
            }
        }
    }
}

fn demo() -> Markup {
    let markdown_input_id = "demo-input";
    html! {
        section #demo .border-b.border-border.bg-surface {
            div.max-w-7xl.mx-auto.px-6.py-16 {
                div.flex.flex-col.sm:flex-row.sm:items-end.sm:justify-between.gap-3.mb-6 {
                    div {
                        h2.text-3xl.font-bold.mb-1 { "Try it." }
                        p.text-secondary {
                            "Live in your browser via WASM. Edit on the left, render on the right."
                        }
                    }
                    div #demo-status .text-sm.text-tertiary.font-mono.text-left.sm:text-right {
                        div {
                            "compiled in "
                            span #demo-stat .text-ink.font-medium { "—" }
                        }
                        div #demo-throughput .text-tertiary.text-xs.mt-1 {
                            "≈ "
                            span #demo-docs-per-sec .text-secondary { "—" }
                            " docs/sec on this machine"
                        }
                    }
                }

                div.grid.grid-cols-1.md:grid-cols-2.border.border-border.rounded-sm.overflow-hidden.bg-paper {
                    div.flex.flex-col.border-b.md:border-b-0.md:border-r.border-border {
                        div.px-4.py-2.text-xs.uppercase.tracking-widest.text-paper.bg-ink.font-bold  {
                            label for=(markdown_input_id) {
                                "Markdown"
                            }
                        }
                        div.relative."flex-1"."min-h-[26rem]"."md:min-h-[36rem]"."max-h-[26rem]"."md:max-h-[36rem]" {
                            pre #demo-highlight .demo-editor-layer.absolute.inset-0.m-0.overflow-auto.pointer-events-none {
                                code {}
                            }
                            textarea id=(markdown_input_id)
                                spellcheck="false"
                                class="demo-editor-layer absolute inset-0 m-0 resize-none w-full h-full bg-transparent text-transparent caret-ink focus:outline-none" {}
                        }
                    }
                    div.flex.flex-col {
                        h3.px-4.py-2.text-xs.uppercase.tracking-widest.text-paper.bg-ink.font-bold {
                            "Rendered HTML"
                        }
                        div #demo-output .prose."min-h-[26rem]"."md:min-h-[36rem]"."max-h-[26rem]"."md:max-h-[36rem]".overflow-auto.p-4.text-sm.leading-relaxed {}
                    }
                }

                p.text-sm.text-tertiary.mt-4.text-right {
                    "Plugins, MDX, and AST inspection in the "
                    a.text-secondary href="/playground/" {
                        "full playground"
                    }
                    "."
                }
            }
        }
    }
}
