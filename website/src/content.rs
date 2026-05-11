use maud::{PreEscaped, Render};
use maudit::content::{
    ContentSources, MarkdownOptions, glob_markdown_with_options, markdown_entry,
};
use maudit::content_sources;
use serde::Deserialize;

#[derive(Deserialize, Eq, PartialEq, PartialOrd, Hash, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum DocsSection {
    GettingStarted,
    Concepts,
    Guides,
    Reference,
}

impl Render for DocsSection {
    fn render(&self) -> PreEscaped<String> {
        match self {
            DocsSection::GettingStarted => PreEscaped("Getting Started".to_string()),
            DocsSection::Concepts => PreEscaped("Concepts".to_string()),
            DocsSection::Guides => PreEscaped("Guides".to_string()),
            DocsSection::Reference => PreEscaped("Reference".to_string()),
        }
    }
}

/// Stable sort order for sections in the sidebar.
impl DocsSection {
    pub fn sort_key(&self) -> u8 {
        match self {
            DocsSection::GettingStarted => 0,
            DocsSection::Concepts => 1,
            DocsSection::Guides => 2,
            DocsSection::Reference => 3,
        }
    }
}

#[markdown_entry]
pub struct DocsContent {
    pub title: String,
    pub description: Option<String>,
    pub section: Option<DocsSection>,
    /// Optional explicit ordering inside a section. Lower is earlier.
    /// Defaults to a high number so unordered pages sort to the end.
    #[serde(default = "default_order")]
    pub order: i32,
}

fn default_order() -> i32 {
    1000
}

pub fn content_sources() -> ContentSources {
    content_sources![
        "docs" => glob_markdown_with_options::<DocsContent>(
            "content/docs/*.md",
            MarkdownOptions {
                highlight_theme: "base16-eighties.dark".into(),
                ..Default::default()
            },
        )
    ]
}
