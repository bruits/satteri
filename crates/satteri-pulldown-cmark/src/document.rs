//! Source-backed semantic document built by the existing grammar traversal.
//! Syntax records remain transient; semantic records have stable, dense IDs.
use satteri_arena::{Document, Mdast};

use crate::Options;
use crate::arena_build::parse_document;

pub type SourceDocument<'a> = Document<'a, Mdast>;

/// Resolve a document using the same grammar and semantic passes as owned arenas.
pub fn parse(
    source: &str,
    options: Options,
    track_positions: bool,
) -> (SourceDocument<'_>, Vec<(usize, String)>) {
    parse_reusing(source, options, track_positions, None)
}

/// Parse using buffers returned by [`SourceDocument::into_reusable`].
pub fn parse_reusing<'a>(
    source: &'a str,
    options: Options,
    track_positions: bool,
    storage: Option<SourceDocument<'static>>,
) -> (SourceDocument<'a>, Vec<(usize, String)>) {
    parse_document(
        source,
        options,
        track_positions,
        storage,
        #[cfg(test)]
        false,
    )
}
