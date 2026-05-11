use maudit::{AssetsOptions, BuildOptions, BuildOutput, coronate, routes};

mod content;
mod docs_layout;
mod layout;
mod routes;

use routes::{Chat, DocsIndex, DocsPage, Index, Playground};

fn main() -> Result<BuildOutput, Box<dyn std::error::Error>> {
    coronate(
        routes![Index, DocsIndex, DocsPage, Playground, Chat],
        content::content_sources(),
        BuildOptions {
            assets: AssetsOptions {
                tailwind_binary_path: "./node_modules/.bin/tailwindcss".into(),
                ..Default::default()
            },
            ..Default::default()
        },
    )
}
