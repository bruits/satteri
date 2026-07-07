use std::path::PathBuf;

use maudit::{AssetsOptions, BuildOptions, BuildOutput, coronate, routes};
use pagefind::api::PagefindIndex;

mod content;
mod docs_layout;
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

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(build_pagefind(output_dir))?;

    Ok(output)
}

async fn build_pagefind(dist: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut index = PagefindIndex::new(None)?;

    let dist_str = dist.clone().to_string_lossy().to_string();
    index.add_directory(dist_str, None).await?;

    let index_dist_str = dist.join("pagefind").to_string_lossy().to_string();
    index.write_files(index_dist_str.into()).await?;

    Ok(())
}
