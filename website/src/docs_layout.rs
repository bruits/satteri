use maud::{Markup, html};
use maudit::route::{PageContext, RouteExt};

use crate::content::{DocsContent, DocsSection};
use crate::layout::{SeoMeta, layout};
use crate::routes::{DocsPage, DocsPageParams};

/// Three-row block on desktop: header (in shared layout), then a two-column
/// grid below — left rail with the section index, right column with the
/// rendered Markdown. On mobile the two columns stack with the rail above.
pub fn docs_layout(
    main: Markup,
    ctx: &mut PageContext,
    seo: Option<SeoMeta>,
) -> Result<Markup, Box<dyn std::error::Error>> {
    ctx.assets.include_script("assets/docs.ts")?;
    ctx.assets.include_script("assets/docs-sidebar.ts")?;
    let sidebar = render_sidebar(ctx);
    layout(
        html! {
            div."md:hidden".sticky.top-0.z-30.bg-paper.border-b.border-border {
                button #docs-sidebar-toggle
                    type="button"
                    aria-label="Open docs navigation"
                    aria-controls="docs-mobile-sidebar"
                    aria-expanded="false"
                    class="flex items-center gap-2 px-6 py-3 text-base text-ink font-medium" {
                    svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true" {
                        path d="M4 6h16M4 12h10M4 18h16" {}
                    }
                    span { "Docs menu" }
                }
            }

            div #docs-mobile-sidebar
                class="md:hidden fixed inset-0 z-40 opacity-0 pointer-events-none transition-opacity bg-ink/40" {
                div #docs-mobile-sidebar-panel
                    class="w-80 max-w-[85%] h-full bg-paper border-r border-border overflow-y-auto -translate-x-full transition-transform" {
                    div.flex.items-center.justify-between.px-4.py-3.border-b.border-border {
                        span.text-xs.uppercase.tracking-widest.text-tertiary.font-bold { "Documentation" }
                        button #docs-sidebar-close
                            type="button"
                            aria-label="Close docs navigation"
                            class="grid place-items-center w-9 h-9 rounded-sm text-secondary hover:text-ink hover:bg-surface cursor-pointer" {
                            svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true" {
                                path d="M6 6l12 12M18 6L6 18" {}
                            }
                        }
                    }
                    div.px-4.py-4 {
                        (sidebar)
                    }
                }
            }

            div.max-w-6xl.mx-auto.px-6.py-8.md:py-12.grid."grid-cols-1"."md:grid-cols-[14rem_1fr]".gap-x-12.gap-y-8 {
                aside.hidden."md:block"."md:sticky"."md:top-8"."md:self-start" {
                    (sidebar)
                }
                article.prose.max-w-3xl.text-lg.leading-relaxed {
                    (main)
                }
            }
        },
        ctx,
        seo,
    )
}

/// Snapshot of one sidebar row, decoupled from the content source so we can
/// sort the rows freely (the entry borrow against `ctx` doesn't need to span
/// the html! block).
struct Row {
    title: String,
    url: String,
    order: i32,
}

fn render_sidebar(ctx: &mut PageContext) -> Markup {
    let current_path = ctx.current_path.clone();

    // First pass: collect titles + URLs + sort keys into owned data so we can
    // group and order without juggling borrows of the content arena.
    let mut prologue: Option<Row> = None;
    let mut by_section: std::collections::HashMap<DocsSection, Vec<Row>> =
        std::collections::HashMap::new();

    let entry_ids: Vec<String> = ctx
        .content::<DocsContent>("docs")
        .entries()
        .map(|e| e.id.clone())
        .collect();

    for id in entry_ids {
        let docs = ctx.content::<DocsContent>("docs");
        let entry = docs.get_entry(&id);
        let data = entry.data(ctx);
        let title = data.title.clone();
        let order = data.order;
        let section = data.section.clone();

        if id == "index" {
            prologue = Some(Row {
                title,
                url: "/docs/".to_string(),
                order,
            });
            continue;
        }

        let Some(section) = section else { continue };
        let url = DocsPage.url(DocsPageParams { slug: id.clone() });
        by_section
            .entry(section)
            .or_default()
            .push(Row { title, url, order });
    }

    let mut ordered: Vec<(DocsSection, Vec<Row>)> = by_section.into_iter().collect();
    ordered.sort_by_key(|(s, _)| s.sort_key());
    for (_, rows) in ordered.iter_mut() {
        rows.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)));
    }

    html! {
        nav.text-base aria-label="Documentation" {
            @if let Some(row) = prologue {
                @let active = row.url == current_path;
                ul.mb-6 {
                    li {
                        (sidebar_link(&row.title, &row.url, active))
                    }
                }
            }
            ul.space-y-6 {
                @for (section, rows) in &ordered {
                    li {
                        h3.text-xs.uppercase.tracking-widest.text-tertiary.mb-2.font-bold {
                            (section)
                        }
                        ul.space-y-px {
                            @for row in rows {
                                @let active = row.url == current_path;
                                li {
                                    (sidebar_link(&row.title, &row.url, active))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn sidebar_link(title: &str, url: &str, active: bool) -> Markup {
    let class = if active {
        "block py-1.5 px-3 -ml-px border-l-2 border-ink text-ink font-medium no-underline"
    } else {
        "block py-1.5 px-3 -ml-px border-l-2 border-border text-secondary hover:text-ink hover:border-tertiary no-underline"
    };
    html! {
        a class=(class) href=(url) { (title) }
    }
}
