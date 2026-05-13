use chrono::{Datelike, Utc};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use maudit::assets::StyleOptions;
use maudit::maud::generator;
use maudit::route::PageContext;

const FLOURISH: &str = include_str!("../assets/flourish.svg");

/// Inline boot script: runs before any CSS resolves so the document is already
/// in the right colour scheme on first paint and we never get a light flash on
/// dark refresh. Reads localStorage first, falls back to the OS preference.
const THEME_INIT: &str = r#"(()=>{try{var s=localStorage.getItem('theme');var d=s?s==='dark':matchMedia('(prefers-color-scheme: dark)').matches;document.documentElement.dataset.theme=d?'dark':'light';}catch(e){}})();"#;

pub struct SeoMeta {
    pub title: String,
    pub description: String,
}

impl Default for SeoMeta {
    fn default() -> Self {
        Self {
            title: "Sätteri".to_string(),
            description: "Sätteri puts flexible JavaScript plugins on top of a fast Rust Markdown / MDX engine. Best of both worlds."
                .to_string(),
        }
    }
}

pub fn layout(
    main: Markup,
    ctx: &mut PageContext,
    seo: Option<SeoMeta>,
) -> Result<Markup, Box<dyn std::error::Error>> {
    layout_with_options(main, ctx, seo, LayoutOptions::default())
}

#[derive(Default)]
pub struct LayoutOptions {
    /// When true, omit the footer and pin body to the viewport height. Used by
    /// the playground so the editor can fill the screen without a scrolling
    /// footer underneath.
    pub fullscreen: bool,
}

pub fn layout_with_options(
    main: Markup,
    ctx: &mut PageContext,
    seo: Option<SeoMeta>,
    opts: LayoutOptions,
) -> Result<Markup, Box<dyn std::error::Error>> {
    ctx.assets
        .include_style_with_options("assets/prin.css", StyleOptions { tailwind: true })?;

    let seo = seo.unwrap_or_default();
    let formatted_title = if seo.title == "Sätteri" {
        seo.title.clone()
    } else {
        format!("{} — Sätteri", seo.title)
    };

    // Fullscreen locks to the viewport on desktop so the playground's editor
    // can fill the screen, but on mobile we let the page scroll so the
    // stacked panels (sidebar / editor / output) are all reachable.
    let body_class = if opts.fullscreen {
        "min-h-screen md:h-screen flex flex-col md:overflow-hidden"
    } else {
        "min-h-screen flex flex-col"
    };

    ctx.assets.include_script("assets/theme.ts")?;
    ctx.assets.include_script("assets/mobile-menu.ts")?;

    Ok(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                (generator())
                link rel="icon" href="/favicon.svg";
                title { (formatted_title) }
                meta name="description" content=(seo.description);
                meta property="og:title" content=(formatted_title);
                meta property="og:description" content=(seo.description);
                meta property="og:type" content="website";
                script { (PreEscaped(THEME_INIT)) }
            }
            body class=(body_class) {
                (header())
                main.flex-1.min-h-0 { (main) }
                @if !opts.fullscreen {
                    (footer())
                }
            }
        }
    })
}

const NAV_LINKS: &[(&str, &str)] = &[
    ("/docs/", "Docs"),
    ("/playground/", "Playground"),
    ("/chat/", "Discord"),
    ("https://github.com/bruits/satteri", "GitHub"),
];

fn header() -> Markup {
    html! {
        header.border-b.border-border.bg-paper.relative.z-40 {
            div.max-w-5xl.mx-auto.px-6.py-5.flex.items-center.justify-between {
                a.no-underline.text-ink.font-logo.text-3xl.leading-none.transition-opacity.hover:opacity-70 href="/" {
                    "Sätteri"
                }
                nav.hidden."md:flex".items-center.gap-6.text-base.text-secondary.relative."top-px" aria-label="Main" {
                    @for (href, label) in NAV_LINKS {
                        a.no-underline.transition-colors.hover:text-ink.hover:underline.decoration-current.underline-offset-4 href=(href) { (label) }
                    }
                    (theme_toggle())
                }
                div."md:hidden".flex.items-center.gap-3 {
                    (theme_toggle())
                    (mobile_menu_button())
                }
            }
            (mobile_menu_panel())
        }
    }
}

fn mobile_menu_button() -> Markup {
    html! {
        button #mobile-menu-button
            type="button"
            aria-label="Toggle menu"
            aria-controls="mobile-menu-panel"
            aria-expanded="false"
            class="grid place-items-center w-11 h-11 rounded-sm text-secondary hover:text-ink hover:bg-surface transition-colors cursor-pointer -translate-y-1" {
            span #mobile-menu-icon-open {
                svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true" {
                    path d="M4 7h16M4 12h16M4 17h16" {}
                }
            }
            span #mobile-menu-icon-close .hidden {
                svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true" {
                    path d="M6 6l12 12M18 6L6 18" {}
                }
            }
        }
    }
}

fn mobile_menu_panel() -> Markup {
    html! {
        div #mobile-menu-panel
            class="md:hidden absolute top-full left-0 right-0 bg-paper border-b border-border opacity-0 -translate-y-2 pointer-events-none transition-all" {
            nav.flex.flex-col {
                @for (href, label) in NAV_LINKS {
                    a class="no-underline text-ink text-xl px-6 py-4 border-b border-border last:border-b-0 hover:bg-surface" href=(href) { (label) }
                }
            }
        }
    }
}

fn theme_toggle() -> Markup {
    html! {
        button
            type="button"
            aria-label="Toggle theme"
            title="Toggle theme"
            class="theme-toggle relative -my-2 ml-1 grid place-items-center w-11 h-11 rounded-sm text-secondary hover:text-ink hover:bg-surface transition-colors cursor-pointer -translate-y-1" {
            // Sun (shown in dark mode → click to go light)
            svg.theme-icon-sun width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                circle cx="12" cy="12" r="4" {}
                path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" {}
            }
            // Moon (shown in light mode → click to go dark)
            svg.theme-icon-moon width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {
                path d="M21 12.79A9 9 0 1 1 11.21 3a7 7 0 0 0 9.79 9.79z" {}
            }
        }
    }
}

fn footer() -> Markup {
    let year = Utc::now().year();
    html! {
        footer.bg-paper {
            div.max-w-3xl.mx-auto.px-6.pt-14.pb-16.text-center {
                div.text-tertiary.mb-10.flex.justify-center."[&_svg]:h-4"."[&_svg]:w-auto".opacity-70 {
                    (PreEscaped(FLOURISH))
                }
                p.text-secondary.leading-relaxed {
                    "A "
                    a.text-ink href="https://bruits.org" { "Bruits" }
                    " project, website built with "
                    a.text-ink href="https://maudit.org" { "Maudit" }
                    "."
                }
                p.text-secondary.leading-relaxed {
                    "Source on "
                    a.text-ink href="https://github.com/bruits/satteri" { "GitHub" }
                    ", install via "
                    a.text-ink href="https://npmx.dev/package/satteri" { "npm" }
                    " or "
                    a.text-ink href="https://crates.io/crates/satteri" { "crates.io" }
                    ", chat on "
                    a.text-ink href="/chat/" { "Discord" }
                    "."
                }
                p.text-secondary.leading-relaxed.mt-3 {
                    "MIT licensed, © " (year) "."
                }
            }
        }
    }
}
