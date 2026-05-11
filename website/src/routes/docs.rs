use maud::{Markup, PreEscaped, html};
use maudit::{content::EntryInner, route::prelude::*};

use crate::content::DocsContent;
use crate::docs_layout::docs_layout;
use crate::layout::SeoMeta;

const ICON_CLAUDE: &str = include_str!("../../assets/icons/claude.svg");
const ICON_OPENAI: &str = include_str!("../../assets/icons/openai.svg");
const ICON_MARKDOWN: &str = include_str!("../../assets/icons/markdown.svg");

#[route("/docs/")]
pub struct DocsIndex;

impl Route for DocsIndex {
    fn render(&self, ctx: &mut PageContext) -> impl Into<RenderResult> {
        let docs = ctx.content::<DocsContent>("docs");
        let entry = docs.get_entry("index");
        let entry_data = entry.data(ctx);

        docs_layout(
            render_entry(entry, ctx),
            ctx,
            Some(SeoMeta {
                title: "Docs".to_string(),
                description: entry_data
                    .description
                    .clone()
                    .unwrap_or_else(|| "Documentation for Sätteri.".to_string()),
            }),
        )
    }
}

#[route("/docs/[slug]/")]
pub struct DocsPage;

#[derive(Params, Clone)]
pub struct DocsPageParams {
    pub slug: String,
}

impl Route<DocsPageParams> for DocsPage {
    fn pages(&self, ctx: &mut DynamicRouteContext) -> Pages<DocsPageParams> {
        let docs = ctx.content::<DocsContent>("docs");
        docs.entries()
            .filter(|entry| entry.id != "index")
            .map(|entry| {
                Page::from_params(DocsPageParams {
                    slug: entry.id.clone(),
                })
            })
            .collect()
    }

    fn render(&self, ctx: &mut PageContext) -> impl Into<RenderResult> {
        let slug = ctx.params::<DocsPageParams>().slug.clone();
        let docs = ctx.content::<DocsContent>("docs");
        let entry = docs.get_entry(&slug);
        let entry_data = entry.data(ctx);

        let seo = SeoMeta {
            title: entry_data.title.clone(),
            description: entry_data
                .description
                .clone()
                .unwrap_or_else(|| format!("{} — Sätteri docs.", entry_data.title)),
        };

        docs_layout(render_entry(entry, ctx), ctx, Some(seo))
    }
}

fn render_entry(entry: &EntryInner<DocsContent>, ctx: &mut PageContext) -> Markup {
    let data = entry.data(ctx);
    let raw = entry.raw_content.clone().unwrap_or_default();
    html! {
        header.mb-10.pb-6.border-b.border-border {
            div.flex.justify-between.items-start.gap-4 {
                div.min-w-0 {
                    @if let Some(section) = &data.section {
                        p.text-xs.uppercase.tracking-widest.text-tertiary.mb-2 { (section) }
                    }
                    h1.text-4xl.font-bold.text-ink.leading-tight { (data.title) }
                }
                (page_actions(&raw))
            }
        }
        (PreEscaped(entry.render(ctx)))
    }
}

fn page_actions(raw_markdown: &str) -> Markup {
    html! {
        div.relative.shrink-0 {
            button #page-actions-button
                type="button"
                aria-haspopup="menu"
                aria-expanded="false"
                class="inline-flex items-center gap-2 px-4 py-2.5 text-sm leading-none border border-border rounded-md bg-paper text-secondary hover:text-ink hover:bg-surface transition-colors cursor-pointer" {
                (icon_copy())
                span.relative."top-px" { "Copy page" }
                svg.text-tertiary width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                    path d="M6 9l6 6 6-6" {}
                }
            }
            div #page-actions-menu
                role="menu"
                class="hidden absolute right-0 mt-2 w-64 z-20 bg-paper border border-border rounded-md shadow-lg text-sm overflow-hidden" {
                (menu_item("action-copy-md", icon_markdown(), "Copy as Markdown", "Copy this page as Markdown"))
                (menu_item("action-claude", icon_claude(), "Open in Claude", "Ask Claude about this page"))
                (menu_item("action-chatgpt", icon_chatgpt(), "Open in ChatGPT", "Ask ChatGPT about this page"))
            }
            template #page-source { (raw_markdown) }
        }
    }
}

fn menu_item(id: &str, icon: Markup, label: &str, sublabel: &str) -> Markup {
    html! {
        button id=(id)
            type="button"
            role="menuitem"
            class="flex w-full items-start gap-3 px-3 py-2.5 text-left text-secondary hover:text-ink hover:bg-surface transition-colors cursor-pointer" {
            span class="shrink-0 mt-0.5 text-tertiary [&_svg]:w-4 [&_svg]:h-4 [&_svg]:block" { (icon) }
            span.flex.flex-col.min-w-0 {
                span.font-medium.text-ink { (label) }
                span.text-xs.text-tertiary { (sublabel) }
            }
        }
    }
}

fn icon_copy() -> Markup {
    html! {
        svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
            rect x="9" y="9" width="13" height="13" rx="2" ry="2" {}
            path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" {}
        }
    }
}

fn icon_markdown() -> Markup {
    html! { (PreEscaped(ICON_MARKDOWN)) }
}

// Brand SVGs from lobehub/lobe-icons. They use `fill="currentColor"` and
// width/height in `1em`, so they inherit the surrounding text color and
// scale with the font-size of the icon container.
fn icon_claude() -> Markup {
    html! { (PreEscaped(ICON_CLAUDE)) }
}

fn icon_chatgpt() -> Markup {
    html! { (PreEscaped(ICON_OPENAI)) }
}
