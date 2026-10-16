use std::path::{Path, PathBuf};

use graphgarden_core::{
    build::build,
    config::{Config, OutputConfig, ParseConfig, SiteConfig},
};
use maudit::{AssetsOptions, BuildOptions, BuildOutput, coronate, routes};
use pagefind::api::PagefindIndex;

mod content;
mod docs_layout;
mod heading;
mod layout;
mod routes;
mod shortcodes;

use routes::{Chat, DocsIndex, DocsPage, Index, Playground};

fn main() -> Result<BuildOutput, Box<dyn std::error::Error>> {
    let maudit_options = BuildOptions {
        assets: AssetsOptions {
            tailwind_binary_path: "./node_modules/.bin/tailwindcss".into(),
            ..Default::default()
        },
        ..Default::default()
    };

    let output_dir = maudit_options.output_dir.clone();

    let output = coronate(
        routes![Index, DocsIndex, DocsPage, Playground, Chat],
        content::content_sources(),
        maudit_options,
    )?;

    build_graphgarden(&output_dir)?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(build_pagefind(output_dir))?;

    Ok(output)
}

fn build_graphgarden(dist: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        site: SiteConfig {
            base_url: "https://satteri.bruits.org/".into(),
            title: "Sätteri".into(),
            description: Some(
                "A fast Markdown and MDX engine with flexible JavaScript plugins".into(),
            ),
            language: Some("en".into()),
        },
        friends: vec![
            "https://bruits.org/".into(),
            "https://maudit.org/".into(),
            "https://erika.florist/".into(),
            "https://garden.bruits.org/".into(),
        ],
        output: OutputConfig {
            dir: dist.to_string_lossy().into_owned(),
        },
        parse: ParseConfig {
            exclude: Some(vec!["chat/index.html".into()]),
            exclude_selectors: Some(vec![
                "header".into(),
                "footer".into(),
                "nav".into(),
                "[data-graphgarden-ignore]".into(),
            ]),
            ..Default::default()
        },
    };

    let json = build(&config)?.to_json()?;

    let well_known_dir = dist.join(".well-known");
    std::fs::create_dir_all(&well_known_dir)?;
    std::fs::write(well_known_dir.join("graphgarden.json"), json)?;

    Ok(())
}

async fn build_pagefind(dist: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut index = PagefindIndex::new(None)?;

    let dist_str = dist.to_string_lossy().to_string();
    index.add_directory(dist_str, None).await?;

    let index_dist_str = dist.join("pagefind").to_string_lossy().to_string();
    index.write_files(index_dist_str.into()).await?;

    Ok(())
}
