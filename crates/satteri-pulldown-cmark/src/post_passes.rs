//! Post-passes that transform the built MDAST tree.
//!
//! `arena_build::parse` produces a structurally complete `Arena<Mdast>`
//! that matches micromark's tokenizer output. The remark ecosystem then
//! layers several `mdast-util-*` / `remark-*` plugins on top to:
//!
//! * recognize bare URLs and emails inside text nodes
//!   ([`gfm_autolink_literal_pass`]),
//! * inline-parse directive labels for backticks and JSX
//!   ([`directive_label_inline_code_pass`], [`directive_label_jsx_pass`]),
//! * mark and unravel MDX-only flow children
//!   ([`mdx_mark_and_unravel`]).
//!
//! Each of those is a self-contained tree-walking transformation that
//! reads / mutates `Arena<Mdast>` after building is finished. They live
//! here so [`arena_build`] stays focused on actually building the arena.

use satteri_arena::{decode_string_ref_data, Arena, ArenaBuilder, Mdast, StringRef};
use satteri_ast::mdast::{codec::LinkData, MdastNodeType};

pub(crate) const MDX_EXPLICIT_JSX_DATA: &[u8] = b"{\"_mdxExplicitJsx\":true}";

pub(crate) fn prev_is_loose_only(bytes: &[u8], ix: usize) -> bool {
    if ix == 0 {
        return false;
    }
    let prev = bytes[ix - 1];
    if prev < 0x80 {
        if prev.is_ascii_alphabetic() {
            return false; // loose itself fails — neither path applies
        }
        let strict = prev.is_ascii_whitespace() || prev.is_ascii_punctuation();
        return !strict;
    }
    match core::str::from_utf8(&bytes[ix.saturating_sub(4)..ix]) {
        Ok(s) => {
            let c = s.chars().last().unwrap_or(' ');
            if c.is_alphabetic() {
                return false;
            }
            let strict = c.is_whitespace() || !c.is_alphanumeric();
            !strict
        }
        Err(_) => false,
    }
}

/// Mirror `mdast-util-gfm-autolink-literal`'s `isCorrectDomain`. Domain must
/// have ≥2 dot-separated parts; the last and penultimate (if non-empty) must
/// contain an ASCII alphanumeric and must not contain `_`. Empty parts are
/// allowed (skipped) so `https://.foo` (parts=[``, `foo`]) and `https://../`
/// (parts=[``, ``, ``]) both pass.
fn is_correct_domain_for_fnr(domain: &[u8]) -> bool {
    let parts: Vec<&[u8]> = domain.split(|&b| b == b'.').collect();
    if parts.len() < 2 {
        return false;
    }
    let check = |p: &[u8]| -> bool {
        if p.is_empty() {
            return true;
        }
        if p.contains(&b'_') {
            return false;
        }
        p.iter().any(|&b| b.is_ascii_alphanumeric())
    };
    check(parts[parts.len() - 1]) && check(parts[parts.len() - 2])
}

/// Mirror `mdast-util-gfm-autolink-literal`'s `splitUrl`: trim trailing chars
/// in `[!"&'),.:;<>?\]}]+` from `raw_end` while balancing `(`/`)`. Returns
/// the new end (≥ `min_end`).
fn split_url_trim_end(bytes: &[u8], min_end: usize, raw_end: usize) -> usize {
    // Find the longest trail at the end.
    let mut trail_start = raw_end;
    while trail_start > min_end {
        let b = bytes[trail_start - 1];
        if matches!(
            b,
            b'!' | b'"'
                | b'&'
                | b'\''
                | b')'
                | b','
                | b'.'
                | b':'
                | b';'
                | b'<'
                | b'>'
                | b'?'
                | b']'
                | b'}'
        ) {
            trail_start -= 1;
        } else {
            break;
        }
    }
    if trail_start == raw_end {
        return raw_end;
    }
    // Now extend back into the trail to balance any unbalanced `(`s in URL.
    let mut url_end = trail_start;
    let url_segment = &bytes[min_end..url_end];
    let mut opens = url_segment.iter().filter(|&&c| c == b'(').count();
    let mut closes = url_segment.iter().filter(|&&c| c == b')').count();
    let trail = &bytes[trail_start..raw_end];
    let mut trail_pos = 0usize;
    while opens > closes {
        // Find next `)` in trail.
        let mut found = None;
        for (i, &c) in trail[trail_pos..].iter().enumerate() {
            if c == b')' {
                found = Some(trail_pos + i);
                break;
            }
        }
        match found {
            Some(p) => {
                let consumed_end = p + 1;
                let segment = &trail[trail_pos..consumed_end];
                opens += segment.iter().filter(|&&c| c == b'(').count();
                closes += segment.iter().filter(|&&c| c == b')').count();
                url_end = trail_start + consumed_end;
                trail_pos = consumed_end;
            }
            None => break,
        }
    }
    url_end
}

pub(crate) fn scan_autolink_literal(
    bytes: &[u8],
    ix: usize,
) -> Option<(usize, usize, usize, String, bool)> {
    // Scheme. remark-gfm's autolink-literal extension handles http(s) and
    // `www.`, but not ftp — so we match that set exactly.
    let (proto_len, is_www) = if bytes[ix..].starts_with(b"http://") {
        (7, false)
    } else if bytes[ix..].starts_with(b"https://") {
        (8, false)
    } else if bytes[ix..].starts_with(b"www.") {
        (4, true)
    } else {
        return None;
    };

    // Two preceding-character rules apply, depending on which path of
    // remark-gfm's autolink-literal pipeline ends up firing:
    //
    //   * micromark's `previousProtocol` (token-level) rejects only when the
    //     previous char is alphabetic — digits, punctuation, ws, and BOF
    //     all pass.
    //   * `mdast-util-gfm-autolink-literal`'s `previous` (find-and-replace,
    //     used as a fallback when the token construct fails) is stricter:
    //     requires whitespace, punctuation, or BOF.
    //
    // We accept the loose check here so we don't miss `0https://…`. The
    // strict version is enforced later when we know whether the
    // micromark path was actually viable (see `prev_loose_only` below).
    let prev_loose_only = if ix > 0 {
        let prev = bytes[ix - 1];
        // micromark's `previousProtocol` rejects only ASCII alphabetic; any
        // non-ASCII byte (including Cyrillic letters etc.) passes the loose
        // check, so the construct can fire after `п` in `_oпhttps://...`.
        let prev_loose_ok = if prev < 0x80 {
            !prev.is_ascii_alphabetic()
        } else {
            true
        };
        if !prev_loose_ok {
            return None;
        }
        let prev_strict_ok = if prev < 0x80 {
            prev.is_ascii_whitespace() || prev.is_ascii_punctuation()
        } else {
            // Find-and-replace's `previous` accepts ws/punct/EOF in Unicode
            // sense. Cyrillic letters are alphabetic, not punctuation, so
            // they fail strict — but pass loose, leaving the construct path.
            match core::str::from_utf8(&bytes[ix.saturating_sub(4)..ix]) {
                Ok(s) => {
                    let c = s.chars().last().unwrap_or(' ');
                    c.is_whitespace() || !c.is_alphanumeric()
                }
                Err(_) => true,
            }
        };
        !prev_strict_ok
    } else {
        false
    };

    // Collect the URL body: everything until whitespace, `<`, ASCII control, or end.
    // Per GFM, valid URLs exclude control characters; matching remark's behavior
    // here avoids autolinking e.g. `http://\x07>` inside a broken `<...>`.
    //
    // micromark's `afterProtocol` rejects when the first byte past `://`
    // is whitespace, control, or Unicode punctuation — but find-and-replace
    // can still accept some of those (e.g. `https://.foo` rejected by
    // construct, accepted by find-and-replace as parts=[``, `foo`]). So we
    // record the construct verdict here and let the later validation decide.
    // (For `www.` the wwwPrefix factory handles its own first-char rules.)
    let construct_first_ok = if is_www {
        true
    } else {
        let first = bytes.get(ix + proto_len).copied();
        match first {
            None => false,
            Some(b) if b <= b' ' || b == 0x7F => false,
            Some(b) if b < 0x80 && b.is_ascii_punctuation() => false,
            _ => true,
        }
    };

    // Special case: micromark's `trail`/`trailBracketAfter` ends the URL at
    // `]` when the next char looks like the start of a CommonMark
    // resource/reference (`(`, `[`, whitespace, EOF). That keeps
    // `https://example.com/?search=](uri)` from gobbling up the trailing
    // `](uri)` even though `]` itself is fine inside a path.
    let mut end = ix + proto_len;
    while end < bytes.len() {
        let b = bytes[end];
        if b <= b' ' || b == 0x7F || b == b'<' {
            break;
        }
        if b == b']' {
            let next = bytes.get(end + 1).copied();
            if matches!(
                next,
                None | Some(b'(')
                    | Some(b'[')
                    | Some(b' ')
                    | Some(b'\t')
                    | Some(b'\n')
                    | Some(b'\r')
            ) {
                break;
            }
        }
        end += 1;
    }

    // Must have at least one char past the scheme.
    if end == ix + proto_len {
        return None;
    }

    // The GFM spec allows `.`, but a `www.` match must have a valid domain
    // (one more `.`-separated segment beyond `www.`). Reject `www.` alone.
    if is_www {
        let rest = &bytes[ix + proto_len..end];
        if rest.is_empty() {
            return None;
        }
    }

    let raw_end = end;

    // Trim trailing punctuation. Set mirrors micromark-gfm-autolink-literal's
    // trail tokenizer: `!"'*,.:;<?]_~` plus unbalanced `)` plus `&;`-
    // terminated entities. Interleaved so that e.g. trailing `")` is fully
    // stripped (`)` via balance, then `"` via the punctuation set).
    loop {
        if end <= ix + proto_len {
            break;
        }
        let last = bytes[end - 1];
        if matches!(
            last,
            b'!' | b'"'
                | b'\''
                | b'*'
                | b','
                | b'.'
                | b':'
                | b';'
                | b'<'
                | b'?'
                | b']'
                | b'_'
                | b'~'
        ) {
            end -= 1;
            continue;
        }
        if last == b')' {
            let segment = &bytes[ix..end];
            let opens = segment.iter().filter(|&&b| b == b'(').count();
            let closes = segment.iter().filter(|&&b| b == b')').count();
            if closes > opens {
                end -= 1;
                continue;
            }
        }
        break;
    }

    // Trim a trailing `;` only when it closes an HTML entity (`&...;`).
    if end > ix + proto_len && bytes[end - 1] == b';' {
        // Walk back looking for `&` before whitespace. If we find `&`, trim the entity.
        let mut j = end - 2;
        while j > ix {
            let c = bytes[j];
            if c == b'&' {
                end = j;
                break;
            }
            if !(c.is_ascii_alphanumeric() || c == b'#') {
                break;
            }
            j -= 1;
        }
    }

    if end <= ix + proto_len {
        return None;
    }

    // The domain (up to first `/`, `?`, `#`, or end) must contain a `.`
    // so that `https://localhost` or `www.` alone don't match — matching
    // remark-gfm's behavior (they DO match http/https/ftp without `.`,
    // but remark-gfm requires a `.` for the literal extension). To align
    // with the reference, allow http/https/ftp without `.` (remark accepts
    // them) but require a `.` for `www.`.
    let body = &bytes[ix + proto_len..end];
    if is_www {
        let domain_end = body
            .iter()
            .position(|&b| matches!(b, b'/' | b'?' | b'#'))
            .unwrap_or(body.len());
        if !body[..domain_end].contains(&b'.') {
            return None;
        }
    }

    // Two paths produce autolinks: micromark's `protocolAutolink` token
    // construct, and `mdast-util-gfm-autolink-literal`'s find-and-replace
    // fallback. Either accepting is enough; we have to evaluate both to
    // know whether to keep this match.
    //
    //   * Construct (`tokenizeDomain`): needs `afterProtocol` to pass
    //     (recorded above), and the domain must contain at least one
    //     alphanumeric/`-` (the `seen` flag) with no `_` in the last or
    //     penultimate dot-segments.
    //   * Find-and-replace (`isCorrectDomain` + `splitUrl`): the strict
    //     `previous` check must pass (recorded as `!prev_loose_only`),
    //     the dot-split must have ≥2 parts whose last/penult segments
    //     contain alphanumeric without `_`, and the trail-trimmed URL
    //     must be non-empty.
    //
    // The two paths also use different trim sets: micromark's `trail`
    // includes `*`, `_`, `~`; find-and-replace's `splitUrl` includes
    // `&`, `>`, `}`. So when only find-and-replace accepts, we re-trim
    // from `raw_end` with the wider set.
    // Domain ends at the first non-domain char. Micromark's
    // `tokenizeDomain` walks only over chars that can appear in a
    // domain (alphanumeric, `-`, `_`, `.`, non-ASCII); anything else
    // ends the domain. Notably `]`, when not at a trail position, is
    // *kept* in the URL body but is NOT part of the domain. So the
    // underscore check applies only to labels left of any such char.
    let construct_domain_end = body
        .iter()
        .position(|&b| {
            !(b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b >= 0x80)
        })
        .unwrap_or(body.len());
    let domain = &body[..construct_domain_end];
    let construct_seen = domain
        .iter()
        .any(|&b| b.is_ascii_alphanumeric() || b == b'-' || b >= 0x80);
    let construct_underscore_ok = {
        let mut last_has_us = false;
        let mut penult_has_us = false;
        for &b in domain {
            if b == b'_' {
                last_has_us = true;
            } else if b == b'.' {
                penult_has_us = last_has_us;
                last_has_us = false;
            }
        }
        !last_has_us && !penult_has_us
    };
    let construct_ok = construct_first_ok && construct_seen && construct_underscore_ok;

    if !construct_ok {
        // Construct rejected. Try find-and-replace.
        if prev_loose_only {
            return None;
        }
        // Use the body extracted via the regex: `[-.\w]+` for domain,
        // `[^ \t\r\n]*` for path (the original collection from `raw_end`
        // already stops only at whitespace/`<`, so we take from `raw_end`
        // and re-derive domain/path).
        let fnr_body = &bytes[ix + proto_len..raw_end];
        // Domain part is `[-.\w]+`: `.`, `_`, `-`, alphanumerics.
        let fnr_domain_end = fnr_body
            .iter()
            .position(|&b| !(b == b'.' || b == b'_' || b == b'-' || b.is_ascii_alphanumeric()))
            .unwrap_or(fnr_body.len());
        let fnr_domain = &fnr_body[..fnr_domain_end];
        if !is_correct_domain_for_fnr(fnr_domain) {
            return None;
        }
        // Re-trim from raw_end with find-and-replace's `splitUrl` set:
        // `[!"&'),.:;<>?\]}]+`, with balanced `)` extension.
        end = split_url_trim_end(bytes, ix + proto_len, raw_end);
        if end <= ix + proto_len {
            return None;
        }
    }

    let url_str = core::str::from_utf8(&bytes[ix..end]).ok()?;
    let full_url = if is_www {
        format!("http://{url_str}")
    } else {
        url_str.to_string()
    };
    Some((ix, raw_end, end, full_url, !construct_ok))
}

#[inline]
fn is_email_local_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'+' | b'-' | b'_')
}

/// GFM extended email autolink. Given `@` at `at_ix`, walk backward for the
/// local-part and forward for the domain. Returns `(start, end, "mailto:...")`.
/// Mirrors `mdast-util-gfm-autolink-literal`: requires a `.` in the domain,
/// the TLD (last dot-segment) must contain at least one letter, and trailing
/// `.`/`-`/`_` are trimmed.
/// Returns (start, end, "mailto:...", retry_needed).
/// `retry_needed` is true when the construct path's prev check failed at
/// max walkback, forcing find-and-replace to try a shorter start. When
/// true, remark emits no position because the construct never tokenized
/// the email. Callers should also treat the email as find-and-replace
/// when the source span contains backslash escapes (text bytes diverge
/// from raw source — micromark would consume the `\X` as an escape token,
/// resetting `self.previous` to `X` (gfmAtext) and rejecting the email
/// construct from firing afterward).
fn scan_email_autolink(bytes: &[u8], at_ix: usize) -> Option<(usize, usize, String, bool)> {
    if at_ix >= bytes.len() || bytes[at_ix] != b'@' {
        return None;
    }
    // Walk backward to find the maximum local-part start. Remark's GFM
    // autolink implementation does not trim any leading local-part
    // punctuation (`+`, `.`, `-`, `_` are all kept), so any non-empty
    // local-part composed of valid email chars is accepted.
    let mut start = at_ix;
    while start > 0 && is_email_local_char(bytes[start - 1]) {
        start -= 1;
    }
    if start == at_ix {
        return None;
    }
    // Two-tier prev check matching micromark's two paths:
    //   - Construct (`emailAutolink`): `previousEmail` rejects `/` (47)
    //     and `gfmAtext` (`+`, `-`, `.`, `_`, alphanumeric).
    //   - Find-and-replace (`(?<=^|\s|\p{P}|\p{S})([-.\w+]+)@`): rejects
    //     `\w` (alphanumeric, `_`) AND `/` (via findEmail's previous(_, true)).
    //
    // At MAX walkback, prev is guaranteed non-local-char (none of `+-._`
    // or alphanumeric, since walkback consumes those). So the construct's
    // gfmAtext check trivially passes — only the `/` exclusion matters.
    let max_prev = if start == 0 {
        None
    } else {
        Some(bytes[start - 1])
    };
    let max_walkback_ok = match max_prev {
        None => true,
        Some(p) => p != b'/',
    };
    let mut retry_needed = !max_walkback_ok;

    if !max_walkback_ok {
        // Find-and-replace retries shorter walkback: advance `start` until
        // prev passes the regex's lookbehind (`^|\s|\p{P}|\p{S}`) AND
        // findEmail's `previous(_, email=true)` allows it (prev != `/`).
        // `_` is in `\p{Pc}` (connector punctuation) so it counts as
        // `\p{P}` for the lookbehind — even though it's also `\w`. Reject
        // only `/` and ASCII alphanumeric here; `+`/`-`/`.`/`_` all pass.
        while start < at_ix {
            let prev_ok = if start == 0 {
                true
            } else {
                let p = bytes[start - 1];
                p != b'/' && !p.is_ascii_alphanumeric()
            };
            if prev_ok {
                break;
            }
            start += 1;
        }
        if start >= at_ix {
            return None;
        }
        retry_needed = true;
    }
    // Forward: scan domain.
    // micromark's email construct accepts `.` as a first domain char
    // (when the `.` came from literal source). Reject is handled in
    // the caller via text-to-source mapping: when source had `\.` (the
    // dot came from an escape), the construct path can't tokenize the
    // email at all, so the caller drops the replacement.
    if at_ix + 1 >= bytes.len() {
        return None;
    }
    let mut end = at_ix + 1;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
            end += 1;
        } else {
            break;
        }
    }
    if end == at_ix + 1 {
        return None;
    }
    // Trim trailing `.` per remark — the find-and-replace regex's
    // `(?:\.[-\w]+)+` segments don't capture a final lone `.` (no `[-\w]+`
    // follows), so the dot stays as text after the email.
    while end > at_ix + 1 && bytes[end - 1] == b'.' {
        end -= 1;
    }
    if end == at_ix + 1 {
        return None;
    }
    // mdast-util-gfm-autolink-literal's findEmail rejects when the domain
    // (label) ends in `-`, ASCII digit, or `_` (the `/[-\d_]$/.test(label)`
    // check). Reject the whole match rather than trim, so e.g.
    // `foo@bar.com-` stays as text, not `<a>foo@bar.com</a>-`.
    {
        let last = bytes[end - 1];
        if matches!(last, b'-' | b'_') || last.is_ascii_digit() {
            return None;
        }
    }
    // Domain must contain at least one `.`.
    let domain = &bytes[at_ix + 1..end];
    let last_dot = domain.iter().rposition(|&b| b == b'.')?;
    // TLD (last dot-segment) must contain at least one ASCII letter.
    let tld = &domain[last_dot + 1..];
    if tld.is_empty() || !tld.iter().any(|&b| b.is_ascii_alphabetic()) {
        return None;
    }
    // Underscore in the last two segments is invalid per remark
    // (`mdast-util-gfm-autolink-literal`'s `emailWithUnderscoreAtEnd` check).
    if tld.contains(&b'_') {
        return None;
    }
    if let Some(second_last_dot) = domain[..last_dot].iter().rposition(|&b| b == b'.') {
        if domain[second_last_dot + 1..last_dot].contains(&b'_') {
            return None;
        }
    } else if domain[..last_dot].contains(&b'_') {
        return None;
    }
    let email_str = core::str::from_utf8(&bytes[start..end]).ok()?;
    Some((start, end, format!("mailto:{email_str}"), retry_needed))
}

/// Re-merge `text + textDirective + text` sibling runs when the text ends
/// with a URL scheme and the directive's name is purely numeric (i.e. a port
/// number that got split off by the directive parser).
///
/// This is the inverse of the split that happens during inline parsing for
/// `http://host:4321/path`: the `:4321` looks like a textDirective, so the
/// inline parser emits `[text("..http://host"), textDirective("4321"), text("/path")]`.
/// GFM autolink would normally consume the whole URL as a single token before
/// the directive parser sees it, but since satteri's autolink runs as a post-
/// pass we reconstruct the original run here so autolink can find the URL.
/// Mirror of mdast-util-gfm-autolink-literal's `isCorrectDomain`: the URL's
/// domain (between `//` and the first `/`, `?`, `#`, or end) must contain a
/// dot to count as a valid autolink. Applied only in strict mode — see the
/// caller.
fn domain_has_dot(url: &str) -> bool {
    let after_scheme = match url.find("://") {
        Some(p) => &url[p + 3..],
        None => url,
    };
    let domain_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    after_scheme[..domain_end].contains('.')
}

/// Fold the bracket-depth running total forward over one string of text.
/// Returns `true` after consuming `s` iff there's a `[` (or `![`) with no
/// matching `]` so far. Backslash-escaped brackets are ignored.
fn update_bracket_depth(was_open: bool, s: &str) -> bool {
    let mut depth: i32 = if was_open { 1 } else { 0 };
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' {
            i += 2;
            continue;
        }
        match c {
            b'[' => depth += 1,
            b']' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth > 0
}

pub(crate) fn merge_directive_port_splits(arena: &mut Arena<Mdast>) {
    // Explicitly skip Link / LinkReference — a bracketed link's label text
    // intentionally preserves `text + textDirective + text` splits (remark
    // keeps them because autolink doesn't recurse into labels).
    let parent_ids: Vec<u32> = (0..arena.len() as u32)
        .filter(|&id| {
            let n = arena.get_node(id);
            matches!(
                MdastNodeType::from_u8(n.node_type),
                Some(
                    MdastNodeType::Paragraph
                        | MdastNodeType::Heading
                        | MdastNodeType::Emphasis
                        | MdastNodeType::Strong
                        | MdastNodeType::Delete
                        | MdastNodeType::TableCell
                )
            )
        })
        .collect();

    for parent_id in parent_ids {
        let children = arena.get_children(parent_id).to_vec();
        if children.len() < 2 {
            continue;
        }
        let mut new_children: Vec<u32> = Vec::with_capacity(children.len());
        let mut i = 0;
        // When a potential link-label `[` remains unclosed in earlier siblings,
        // remark's autolink-literal never tokenizes URLs in the following text
        // and its post-transformer rejects no-dot domains. Merging back would
        // then resurrect URLs remark deliberately leaves alone (see
        // `docs/src/content/docs/ru/guides/testing.mdx` in the conformance
        // check). Track the running bracket depth across preceding siblings so
        // we can bail when we're inside a broken label attempt.
        let mut unmatched_open_bracket = false;
        while i < children.len() {
            let text_id = children[i];
            let text_node = arena.get_node(text_id);
            // Track bracket depth across every text node we visit so the
            // unmatched-`[` gate below sees a correct running total.
            let is_text = text_node.node_type == MdastNodeType::Text as u8;
            if is_text {
                let d = arena.get_type_data(text_id);
                if !d.is_empty() {
                    let s = arena.get_str(StringRef::from_bytes(d));
                    unmatched_open_bracket = update_bracket_depth(unmatched_open_bracket, s);
                }
            }
            // Need a text node whose value ends with `://<host>` (no path yet).
            if !is_text || i + 1 >= children.len() {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            if unmatched_open_bracket {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            let dir_id = children[i + 1];
            let dir_node = arena.get_node(dir_id);
            if dir_node.node_type != MdastNodeType::TextDirective as u8 {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            // Directive name must be all ASCII digits (port number).
            let dir_data = arena.get_type_data(dir_id);
            if dir_data.len() < 8 {
                new_children.push(text_id);
                i += 1;
                continue;
            }
            let dir_name_sr = StringRef::from_bytes(&dir_data[..8]);
            let dir_name = arena.get_str(dir_name_sr).to_string();
            if dir_name.is_empty() || !dir_name.bytes().all(|b| b.is_ascii_digit()) {
                new_children.push(text_id);
                i += 1;
                continue;
            }

            // Text must end with `://<host>` — check by looking for `://`
            // after the last whitespace and then any non-whitespace host.
            let text_data = arena.get_type_data(text_id);
            let text_sr = StringRef::from_bytes(text_data);
            let text_val = arena.get_str(text_sr).to_string();
            let looks_like_url_host = {
                let after_ws = text_val
                    .rsplit(|c: char| c.is_whitespace())
                    .next()
                    .unwrap_or("");
                after_ws.contains("://")
            };
            if !looks_like_url_host {
                new_children.push(text_id);
                i += 1;
                continue;
            }

            // Build merged value. Trailing text (i+2) is merged too if present
            // and starts with a URL-path char, or we leave it standalone.
            let mut merged = text_val;
            merged.push(':');
            merged.push_str(&dir_name);

            let mut consumed = 2; // text + directive
            if i + 2 < children.len() {
                let after_id = children[i + 2];
                let after_node = arena.get_node(after_id);
                if after_node.node_type == MdastNodeType::Text as u8 {
                    let after_data = arena.get_type_data(after_id);
                    let after_sr = StringRef::from_bytes(after_data);
                    let after_val = arena.get_str(after_sr);
                    merged.push_str(after_val);
                    consumed = 3;
                }
            }

            let merged_sr = arena.alloc_string(&merged);
            let text_node_start = arena.get_node(text_id).start_offset;
            let last_id = children[i + consumed - 1];
            let last_node = arena.get_node(last_id);
            let end_offset = last_node.end_offset;
            let end_line = last_node.end_line;
            let end_column = last_node.end_column;
            let start_line = arena.get_node(text_id).start_line;
            let start_column = arena.get_node(text_id).start_column;

            // Reuse the first text node as the merged one.
            arena.set_type_data(text_id, &merged_sr.as_bytes());
            arena.set_position(
                text_id,
                text_node_start,
                end_offset,
                start_line,
                start_column,
                end_line,
                end_column,
            );
            // The leading text's brackets were already folded into
            // `unmatched_open_bracket` at the top of the loop; fold in the
            // remaining text (if any) from the trailing sibling we consumed.
            if consumed == 3 {
                let tail_sr = StringRef::from_bytes(arena.get_type_data(children[i + 2]));
                let tail = arena.get_str(tail_sr);
                unmatched_open_bracket = update_bracket_depth(unmatched_open_bracket, tail);
            }
            new_children.push(text_id);
            i += consumed;
        }
        if new_children.len() != children.len() {
            arena.set_children(parent_id, &new_children);
        }
    }
}

pub(crate) fn gfm_autolink_literal_pass(arena: &mut Arena<Mdast>) {
    let len = arena.len() as u32;
    // First collect the set of Text nodes containing URL candidates to avoid
    // mutating while iterating in a way that shifts indices. Alongside each
    // candidate we track whether we're inside a broken link-label attempt —
    // remark's autolink-literal skips such text during tokenization, and its
    // post-transformer then requires a `.` in the domain to match, so we mirror
    // that "require dot" rule only in the broken-label case.
    let mut candidates: Vec<(u32, bool)> = Vec::new();
    // Per-parent running bracket depth. Indexed by node id, sized to the
    // arena once: avoids the per-text-node HashMap entry/get hot in profiles.
    let mut bracket_depth_by_parent: Vec<i32> = vec![0; len as usize];
    let text_ty = MdastNodeType::Text as u8;
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != text_ty {
            continue;
        }
        let parent_id = node.parent;
        if parent_id == u32::MAX || parent_id >= len {
            continue;
        }
        let parent_type = MdastNodeType::from_u8(arena.get_node(parent_id).node_type);
        // Skip text inside link/linkReference/image/imageReference (mirrors
        // `mdast-util-gfm-autolink-literal`'s `{ignore: ['link', 'linkReference']}`,
        // and also avoids nesting links inside image alt text), code, imports,
        // expressions, or frontmatter.
        if matches!(
            parent_type,
            Some(
                MdastNodeType::Link
                    | MdastNodeType::LinkReference
                    | MdastNodeType::Image
                    | MdastNodeType::ImageReference
                    | MdastNodeType::InlineCode
                    | MdastNodeType::Code
                    | MdastNodeType::MdxjsEsm
                    | MdastNodeType::MdxFlowExpression
                    | MdastNodeType::MdxTextExpression
                    | MdastNodeType::Yaml
                    | MdastNodeType::Toml
            )
        ) {
            continue;
        }
        // Walk up to find the inline-block ancestor (Paragraph/Heading/
        // TableCell). Brackets must propagate across nested inline parents
        // like Emphasis/Strong/Delete — `*![fw*https://example.com` opens
        // `![` inside Emphasis, but micromark's `previousUnbalanced` sees
        // an open labelImage when `https` starts, so the construct path
        // is suppressed (find-and-replace runs without position).
        let mut block_id = parent_id;
        loop {
            let ty = MdastNodeType::from_u8(arena.get_node(block_id).node_type);
            if matches!(
                ty,
                Some(MdastNodeType::Paragraph | MdastNodeType::Heading | MdastNodeType::TableCell)
            ) {
                break;
            }
            let p = arena.get_node(block_id).parent;
            if p == u32::MAX || p >= len {
                break;
            }
            block_id = p;
        }
        let block_type = MdastNodeType::from_u8(arena.get_node(block_id).node_type);
        let tracks_brackets = matches!(
            block_type,
            Some(MdastNodeType::Paragraph | MdastNodeType::Heading | MdastNodeType::TableCell)
        );
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        let bytes = text.as_bytes();
        let mut matched = false;
        let mut search_from = 0;
        while let Some(rel) = memchr::memchr3(b'h', b'w', b'@', &bytes[search_from..]) {
            let i = search_from + rel;
            let b = bytes[i];
            if (b == b'h' || b == b'w') && scan_autolink_literal(bytes, i).is_some() {
                matched = true;
                break;
            }
            if b == b'@' && scan_email_autolink(bytes, i).is_some() {
                matched = true;
                break;
            }
            search_from = i + 1;
        }
        if tracks_brackets {
            let slot = &mut bracket_depth_by_parent[block_id as usize];
            let was_open = *slot > 0;
            if matched {
                candidates.push((id, was_open));
            }
            if memchr::memchr2(b'[', b']', bytes).is_some() {
                let now_open = update_bracket_depth(was_open, text);
                *slot = if now_open { 1 } else { 0 };
            }
        } else if matched {
            candidates.push((id, false));
        }
    }

    for (node_id, strict) in candidates {
        split_text_with_autolinks(arena, node_id, strict);
    }
}

fn split_text_with_autolinks(arena: &mut Arena<Mdast>, text_id: u32, strict_domain: bool) {
    let node = arena.get_node(text_id);
    let start_offset = node.start_offset;
    let end_offset = node.end_offset;
    let start_line = node.start_line;
    let start_column = node.start_column;
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    // When a URL's bytes don't appear verbatim in the source span this text
    // node covers, the source must have been rewritten (e.g. `\X` consumed
    // a backslash escape inside the URL). micromark's literalAutolink
    // tokenizer operates on the raw source, so a rewrite inside the URL
    // means the construct failed at the token level and remark fell back
    // to `mdast-util-find-and-replace`, which emits position-less nodes.
    // Use a per-replacement check: a stale full-text comparison would
    // suppress positions for `=h _ 7\<https://...>` where the `\<` is
    // *outside* the URL and micromark would still have tokenized.
    let source_slice: Vec<u8> = {
        let source_bytes = arena.source.as_bytes();
        if (start_offset as usize) < source_bytes.len()
            && (end_offset as usize) <= source_bytes.len()
        {
            source_bytes[start_offset as usize..end_offset as usize].to_vec()
        } else {
            Vec::new()
        }
    };
    let url_in_source = |url_bytes: &[u8]| -> bool {
        !source_slice.is_empty()
            && source_slice
                .windows(url_bytes.len())
                .any(|w| w == url_bytes)
    };

    // Map text-byte positions (post-escape) back to source-byte positions
    // (raw). When a URL spans `\X` in source it shows up as `X` in text;
    // remark keeps the raw `\X` in both the URL value and the displayed
    // link text (the construct tokenizes raw source). Used below to
    // recover the original bytes when `url_in_source` says the URL was
    // rewritten by an escape mid-URL.
    //
    // Returns `None` if the bytes diverge in any way other than backslash
    // escapes; we then fall back to text bytes (better-than-nothing).
    let text_to_source: Option<Vec<usize>> = {
        // Fast-path: text and source slice are identical → identity map,
        // skip the `bytes.len() + 1` allocation and full walk.
        if !source_slice.is_empty() && source_slice == bytes {
            let mut map = Vec::with_capacity(bytes.len() + 1);
            for i in 0..=bytes.len() {
                map.push(i);
            }
            Some(map)
        } else {
            let mut map = Vec::with_capacity(bytes.len() + 1);
            let mut s = 0usize;
            let mut t = 0usize;
            let mut ok = true;
            while t < bytes.len() {
                if s >= source_slice.len() {
                    ok = false;
                    break;
                }
                if source_slice[s] == b'\\'
                    && s + 1 < source_slice.len()
                    && source_slice[s + 1].is_ascii_punctuation()
                {
                    s += 1;
                }
                // Skip trailing whitespace before a line ending — micromark
                // trims these out of the inline text but the source bytes
                // still occupy positions. Without skipping, the map would
                // mismatch (text `\n` at where source has ` `) and the whole
                // map would be discarded.
                while s < source_slice.len()
                    && matches!(source_slice[s], b' ' | b'\t')
                    && bytes[t] != source_slice[s]
                {
                    let mut peek = s + 1;
                    while peek < source_slice.len() && matches!(source_slice[peek], b' ' | b'\t') {
                        peek += 1;
                    }
                    if peek < source_slice.len() && matches!(source_slice[peek], b'\n' | b'\r') {
                        s += 1;
                    } else {
                        break;
                    }
                }
                // Also skip leading whitespace right after a line ending —
                // continuation lines in a paragraph drop their indentation
                // when collapsed into the inline text (e.g. `z\n i}@…` →
                // text `z\ni}@…`). Without skipping, source position would
                // diverge from text position past the wrap.
                if t > 0
                    && matches!(bytes[t - 1], b'\n' | b'\r')
                    && matches!(source_slice[s], b' ' | b'\t')
                    && bytes[t] != source_slice[s]
                {
                    while s < source_slice.len() && matches!(source_slice[s], b' ' | b'\t') {
                        s += 1;
                    }
                }
                // Blockquote-prefix skip: after a line ending, the source
                // may carry a `>` marker (optionally with `>` chains and
                // trailing whitespace) that's stripped from the collapsed
                // inline text. Without this skip, the map would diverge as
                // soon as a blockquote continuation line carried real
                // content (`>bar\n> baz` collapsed to `bar\nbaz` — text[4]
                // is `b`, source[4] is `>`).
                if t > 0
                    && matches!(bytes[t - 1], b'\n' | b'\r')
                    && source_slice[s] == b'>'
                    && bytes[t] != b'>'
                {
                    while s < source_slice.len() && source_slice[s] == b'>' {
                        s += 1;
                        while s < source_slice.len() && matches!(source_slice[s], b' ' | b'\t') {
                            s += 1;
                        }
                    }
                }
                if s >= source_slice.len() || source_slice[s] != bytes[t] {
                    ok = false;
                    break;
                }
                map.push(s);
                s += 1;
                t += 1;
            }
            if ok {
                map.push(s);
                Some(map)
            } else {
                None
            }
        }
    };

    // Returns the source-byte position where text[pos] BEGINS being
    // represented — i.e. the byte AFTER the previous text byte's source
    // position. For pos == 0 returns 0, so the first emitted chunk's
    // source span covers any leading escape bytes (e.g. `\[foo` text `[foo`
    // spans source 0..2 for the leading `[`, not just 1..2).
    let chunk_src_pos = |pos: usize| -> usize {
        if let Some(map) = text_to_source.as_ref() {
            if pos == 0 {
                0
            } else {
                map[pos - 1] + 1
            }
        } else {
            pos
        }
    };

    // (start, raw_end, end, url, is_email, fnr_only).
    // - is_email: URL vs email path. Emails skip raw-source URL recovery
    //   (the URL value is the `mailto:` prefix + decoded text, not source
    //   bytes).
    // - fnr_only: true when only the find-and-replace path accepted (the
    //   construct path was rejected — e.g. email retry needed, or URL with
    //   first-char punct accepted only via `isCorrectDomain` + `splitUrl`).
    //   Combined with an escape-in-span check, this gates position emission:
    //   find-and-replace doesn't emit positions, so we suppress them here.
    let mut replacements: Vec<(usize, usize, usize, String, bool, bool)> = Vec::new();
    let mut i = 0;
    while let Some(rel) = memchr::memchr3(b'h', b'w', b'@', &bytes[i..]) {
        i += rel;
        let b = bytes[i];
        if b == b'h' || b == b'w' {
            if let Some((s, raw_e, e, url, fnr_only)) = scan_autolink_literal(bytes, i) {
                if strict_domain && !domain_has_dot(&url) {
                    i += 1;
                    continue;
                }
                replacements.push((s, raw_e, e, url, false, fnr_only));
                i = raw_e;
                continue;
            }
        } else if let Some((s, e, url, retry_needed)) = scan_email_autolink(bytes, i) {
            // Drop entirely when the first domain byte came from a
            // backslash-escape in source (`2@\.baz` text → `2@.baz`,
            // but micromark's construct can't tokenize past the escape
            // and find-and-replace's regex `[-\w]+\.` rejects `.` as
            // first domain char — so no link is emitted).
            let drop_email = text_to_source.as_ref().is_some_and(|map| {
                let after_at = i + 1;
                if after_at >= bytes.len() {
                    return false;
                }
                let src_after_at = map[after_at];
                src_after_at > 0
                    && source_slice[src_after_at - 1] == b'\\'
                    && (after_at == 0 || bytes[after_at - 1] != b'\\')
            });
            if !drop_email
                && replacements
                    .last()
                    .is_none_or(|&(_, _, prev_e, _, _, _)| s >= prev_e)
            {
                replacements.push((s, e, e, url, true, retry_needed));
            }
            i = e;
            continue;
        }
        i += 1;
    }

    if replacements.is_empty() {
        return;
    }

    // `outer_open` is the block-level bracket-open state carried over from
    // preceding sibling text (e.g. `*![fw*https://…` — the `[` sits inside
    // an Emphasis text node, not in our bytes, but micromark's
    // `previousUnbalanced` still sees the open labelImage when we reach
    // the URL). Tracked via `strict_domain` from the candidate pass.
    let outer_open = strict_domain;
    let bracket_open_at = |s: usize| -> bool {
        let mut depth: i32 = if outer_open { 1 } else { 0 };
        let mut j = 0;
        while j < s {
            let c = bytes[j];
            if c == b'\\' {
                j += 2;
                continue;
            }
            // Source-backslash skip: a `[`/`]` in text that came from an
            // escaped `\[`/`\]` in source must not count toward bracket
            // balance — micromark's `previousUnbalanced` sees an escape
            // token, not a labelLink. Without this check, an escaped `\[`
            // upstream would wrongly trigger find-and-replace's fragmented
            // text emission for the literal autolink.
            if c == b'[' || c == b']' {
                if let Some(map) = text_to_source.as_ref() {
                    let src_pos = map[j];
                    if src_pos > 0 && source_slice[src_pos - 1] == b'\\' {
                        // Confirm the backslash is itself unescaped (`\\` is
                        // a different escape, but rare here — treat any prior
                        // `\` as escaping).
                        j += 1;
                        continue;
                    }
                }
            }
            match c {
                b'[' => depth += 1,
                b']' if depth > 0 => depth -= 1,
                _ => {}
            }
            j += 1;
        }
        depth > 0
    };

    // Both autolink paths fail when the preceding char passes only the loose
    // (token-level) check AND we're inside an unclosed `[`. micromark's
    // `previousUnbalanced` rejects the construct, and find-and-replace's
    // strict `previous` (whitespace/punct/start) rejects the fallback. Drop
    // those replacements entirely (e.g. `[0https://example.com/...` → no link).
    replacements
        .retain(|&(s, _, _, _, _, _)| !(bracket_open_at(s) && prev_is_loose_only(bytes, s)));
    if replacements.is_empty() {
        return;
    }

    // Remark-gfm keeps the trailing trim-back chars (e.g. `),` stripped from
    // the URL) as their own text node — rather than merging with the post-URL
    // tail — when the preceding text contains an unclosed `[` or `![`. This
    // mirrors a micromark quirk where the failed label/link attempt around the
    // autolink leaves fragmented text tokens that never coalesce.
    let preceded_by_open_bracket: Vec<bool> = replacements
        .iter()
        .map(|&(s, _, _, _, _, _)| bracket_open_at(s))
        .collect();

    // When preceded by an unbalanced `[`/`![`, micromark's `previousUnbalanced`
    // suppresses the construct path entirely — only find-and-replace can
    // still accept. find-and-replace's `isCorrectDomain` requires ≥2 dot
    // segments with alphanumeric content. Drop URL replacements whose
    // domain doesn't pass that check (emails go through findEmail which
    // doesn't require this).
    let to_drop: Vec<bool> = replacements
        .iter()
        .enumerate()
        .map(|(idx, (s, _, _, _, is_email, _))| {
            if *is_email || !preceded_by_open_bracket[idx] {
                return false;
            }
            // Extract domain from the URL: between `proto://` and first `/?#`.
            let url_bytes = &bytes[*s..];
            let proto_len = if url_bytes.starts_with(b"http://") {
                7
            } else if url_bytes.starts_with(b"https://") {
                8
            } else {
                0
            };
            let domain_start = *s + proto_len;
            let domain_end = bytes[domain_start..]
                .iter()
                .position(|&b| {
                    matches!(b, b'/' | b'?' | b'#' | b' ' | b'\t' | b'\n' | b'\r' | b'<')
                })
                .map(|p| domain_start + p)
                .unwrap_or(bytes.len());
            !is_correct_domain_for_fnr(&bytes[domain_start..domain_end])
        })
        .collect();
    let mut keep_iter = to_drop.iter();
    replacements.retain(|_| !*keep_iter.next().unwrap());
    if replacements.is_empty() {
        return;
    }
    let preceded_by_open_bracket: Vec<bool> = replacements
        .iter()
        .map(|&(s, _, _, _, _, _)| bracket_open_at(s))
        .collect();

    // When the literalAutolink construct is suppressed (`previousUnbalanced`),
    // remark falls back to find-and-replace whose `splitUrl` trims a
    // different set than micromark's tokenizer:
    //   * construct trims `*`, `_`, `~` (and `'`); splitUrl does not.
    //   * splitUrl trims `>`, `}`; construct does not.
    // Both trim `!"&'),.:;<?]` and balanced `)`. To match remark, recompute
    // the URL end from `raw_e` using `split_url_trim_end`, which uses the
    // wider splitUrl set. Any chars the construct stripped that splitUrl
    // would have kept get restored (e.g. trailing `_` in `<URL_**`).
    for (idx, repl) in replacements.iter_mut().enumerate() {
        if !preceded_by_open_bracket[idx] {
            continue;
        }
        let (s, raw_e, ref mut e, ref mut url, is_email, _retry) = *repl;
        if is_email {
            continue;
        }
        let proto_len = if bytes[s..].starts_with(b"http://") {
            7
        } else if bytes[s..].starts_with(b"https://") {
            8
        } else if bytes[s..].starts_with(b"www.") {
            4
        } else {
            0
        };
        let min_end = s + proto_len;
        // Stop the URL at the first backtick that opens a valid code
        // span — micromark tokenizes code spans before find-and-replace
        // runs, so the backticks (and content between them) aren't in
        // the path bytes that splitUrl sees. Without this, the URL
        // gobbles `<URL>.\`baz\`` instead of letting the trailing
        // `\`baz\`` become a separate code span.
        let mut search_end = raw_e;
        let mut i = min_end;
        while i < search_end {
            if bytes[i] == b'`' {
                let mut run = 1;
                let mut j = i + 1;
                while j < search_end && bytes[j] == b'`' {
                    run += 1;
                    j += 1;
                }
                let mut k = j;
                let mut matched = false;
                while k < search_end {
                    if bytes[k] == b'`' {
                        let mut close = 1;
                        let mut m = k + 1;
                        while m < search_end && bytes[m] == b'`' {
                            close += 1;
                            m += 1;
                        }
                        if close == run {
                            matched = true;
                            break;
                        }
                        k = m;
                    } else {
                        k += 1;
                    }
                }
                if matched {
                    search_end = i;
                    break;
                }
                i = j;
            } else {
                i += 1;
            }
        }
        let new_end = split_url_trim_end(bytes, min_end, search_end);
        if new_end != *e {
            let url_bytes = &bytes[s..new_end];
            if let Ok(url_str) = core::str::from_utf8(url_bytes) {
                *url = if bytes[s..].starts_with(b"www.") {
                    format!("http://{url_str}")
                } else {
                    url_str.to_string()
                };
                *e = new_end;
            }
        }
    }

    // Find-and-replace's URL regex (`[^ \t\r\n]*` for path) also consumes
    // trailing chars that micromark's construct excludes from the URL — most
    // notably `]`, which my scanner stops BEFORE when followed by a space-
    // like char. When the URL was emitted via the find-and-replace path
    // (preceded_by_open_bracket), `splitUrl` then trims those chars and
    // emits them as a separate text node. Extend `raw_e` forward past trim
    // chars so the post-URL chunk emission can split URL trail from the
    // surrounding text. E.g. `[https://foo.barq] x` should produce
    // `[`, LINK, `]`, ` x` — four nodes.
    for (idx, repl) in replacements.iter_mut().enumerate() {
        if !preceded_by_open_bracket[idx] {
            continue;
        }
        // Emails: mdast-util-gfm-autolink-literal's findEmail returns a
        // Link directly with no `trail` second-element split. So the
        // chars after the email stay in the surrounding text — no extra
        // text-node split. Only URL/www literals go through splitUrl.
        if repl.4 {
            continue;
        }
        let (_s, raw_e, e, _url, _is_email, _retry) =
            (repl.0, &mut repl.1, repl.2, &repl.3, repl.4, repl.5);
        let mut walker = e;
        while walker < bytes.len() {
            let b = bytes[walker];
            if matches!(
                b,
                b'!' | b'"'
                    | b'&'
                    | b'\''
                    | b','
                    | b'.'
                    | b':'
                    | b';'
                    | b'<'
                    | b'>'
                    | b'?'
                    | b']'
                    | b'}'
            ) {
                walker += 1;
            } else {
                break;
            }
        }
        if walker > *raw_e {
            *raw_e = walker;
        }
    }

    // Build the replacement nodes in order.
    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;

    // micromark's autolink-literal construct is skipped when there's an open
    // `[`/`![` ahead of it (`previousUnbalanced` in the extension); in that
    // case `mdast-util-gfm-autolink-literal` falls back to a post-parse
    // find-and-replace pass that emits bare `{type, value}` / `{type, url, ...}`
    // objects without position. Mirror that here: emit positioned nodes when
    // micromark would have tokenized the autolink directly, and leave them
    // position-less (zero-init) when we're standing in for find-and-replace.
    // True only when a replacement falls back to find-and-replace (no
    // raw-source recovery possible). URL replacements with escapes that
    // we recovered to source bytes still have positions, so they don't
    // count as "rewritten" for the trailing-chunk position decision.
    // True when any replacement went through find-and-replace and we
    // couldn't recover raw source bytes — in those cases remark emits
    // the trailing post-URL chunk position-less. Includes:
    //   - URL with escape and no source-mapping fallback (text rewrote).
    //   - URL accepted only by find-and-replace (construct rejected).
    //   - Email with retry-needed (max-walkback prev failed).
    //   - Email with `\X` escape inside the local part.
    let any_url_rewritten = replacements.iter().any(|(s, _, e, _, is_email, fnr_only)| {
        let mismatch = !url_in_source(&bytes[*s..*e]);
        if *is_email {
            let escape_before = text_to_source.as_ref().is_some_and(|map| {
                let src_s = map[*s];
                src_s > 0 && source_slice[src_s - 1] == b'\\' && (*s == 0 || bytes[*s - 1] != b'\\')
            });
            return *fnr_only || mismatch || escape_before;
        }
        *fnr_only || (mismatch && text_to_source.is_none())
    });

    // Walk a byte slice (text or source) to compute (line, column) for any
    // byte offset within it; lines increment on `\n` (or `\r`/`\r\n`),
    // columns reset. Without this, multi-line text nodes (paragraphs that
    // contain a soft-wrap) would attribute the autolink to the parent
    // text's start_line, off by however many wraps preceded it in source.
    let line_col_in = |slice: &[u8], pos: usize| -> (u32, u32) {
        let mut line = start_line;
        let mut col = start_column;
        let mut i = 0;
        while i < pos {
            let b = slice[i];
            if b == b'\n' {
                line += 1;
                col = 1;
                i += 1;
            } else if b == b'\r' {
                line += 1;
                col = 1;
                i += 1;
                if i < pos && slice[i] == b'\n' {
                    i += 1;
                }
            } else if (b & 0xC0) != 0x80 {
                col += 1;
                i += 1;
            } else {
                i += 1;
            }
        }
        (line, col)
    };
    let line_col_at_src = |src_pos: usize| line_col_in(&source_slice, src_pos);

    for (idx, (s, raw_e, e, url, is_email, fnr_only)) in replacements.into_iter().enumerate() {
        // When the URL spans a backslash-escape in source, recover the
        // raw source bytes (`\[\>` not `[>`). micromark's literalAutolink
        // construct tokenizes raw source, so both the URL value and the
        // displayed text keep the backslashes. The construct succeeded,
        // so positions ARE emitted, just relative to the source span.
        //
        // Skip the recovery when:
        //  - is_email: the email construct can't tokenize `\` in the
        //    local-part, so find-and-replace runs on text bytes.
        //  - preceded_by_open_bracket: the construct is suppressed by
        //    `previousUnbalanced`, so find-and-replace runs on text bytes
        //    (e.g. `[=https://example.com?find=\*` → URL `...?find=*`).
        //  - fnr_only: the URL was accepted only by find-and-replace
        //    (construct's afterProtocol/seen/underscore rules rejected
        //    it), so the URL value is the text bytes, not source bytes.
        let raw_source_url = if !is_email
            && !preceded_by_open_bracket[idx]
            && !fnr_only
            && !url_in_source(&bytes[s..e])
        {
            text_to_source.as_ref().and_then(|map| {
                core::str::from_utf8(&source_slice[map[s]..map[e]])
                    .ok()
                    .map(str::to_string)
            })
        } else {
            None
        };
        let url_for_node: String = raw_source_url.clone().unwrap_or(url);
        let displayed: &str = raw_source_url.as_deref().unwrap_or(&text[s..e]);

        // Position emission rules:
        // - `preceded_by_open_bracket`: micromark suppresses the URL
        //   construct, find-and-replace runs → no position.
        // - `fnr_only`: construct rejected, only find-and-replace accepted
        //   → no position (find-and-replace doesn't propagate positions).
        // - Email path: position only when the construct path applies —
        //   no retry was needed AND text bytes appear in source (no
        //   backslash escape in span). Otherwise find-and-replace → no
        //   position.
        // - URL path: position when raw source recovered (construct on
        //   source bytes) OR text bytes in source (construct on text).
        // Email construct also fails when the byte directly before the
        // local-part in source is a backslash that introduced an escape
        // (e.g. `\+@bar.example.com` → email span text starts at `+` but
        // source position is past the `\` that was consumed). micromark
        // tokenizes `\+` as characterEscape, leaving the email construct
        // unable to walk back across the escape boundary. find-and-replace
        // then accepts the email — without position.
        //
        // Only an *actual* escape blocks the construct — `\e` (where `e`
        // isn't punctuation) leaves the `\` literal, so the construct is
        // unaffected (`3\e-gdafoo@…` still emits position on the email).
        // Detect a real escape via the text-to-source map: if `\` was
        // consumed as escape, map[s] skips past it (map[s] > previous +
        // 1); if literal, the `\` shows up in text too and our `s` would
        // include it. Equivalently: `\` immediately before the local part
        // in source AND that `\` doesn't appear at text[s-1].
        let email_escape_before = is_email
            && text_to_source.as_ref().is_some_and(|map| {
                let src_s = map[s];
                src_s > 0 && source_slice[src_s - 1] == b'\\' && (s == 0 || bytes[s - 1] != b'\\')
            });
        let email_fnr =
            is_email && (fnr_only || !url_in_source(&bytes[s..e]) || email_escape_before);
        let with_position = !preceded_by_open_bracket[idx]
            && !fnr_only
            && !email_fnr
            && (raw_source_url.is_some() || url_in_source(&bytes[s..e]));
        // Use source-byte offsets for the link/displayed-text spans when we
        // recovered raw source bytes; the `s..e` text positions don't
        // correspond to where the URL actually sits in source.
        let span_offsets = if raw_source_url.is_some() {
            text_to_source.as_ref().map(|map| (map[s], map[e]))
        } else {
            None
        };

        if s > cursor {
            let chunk = &text[cursor..s];
            let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
            let chunk_sr = arena.alloc_string(chunk);
            arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
            if with_position {
                // Source positions: chunk_src_pos(0) = 0 (so a leading
                // backslash-escape inside the chunk is included in span);
                // chunk_src_pos(s) gives where the email/URL starts in
                // source, accounting for any escapes consumed before it.
                let cur_src = chunk_src_pos(cursor);
                let end_src = chunk_src_pos(s);
                let (sl, sc) = line_col_at_src(cur_src);
                let (el, ec) = line_col_at_src(end_src);
                arena.set_position(
                    new_text_id,
                    start_offset + cur_src as u32,
                    start_offset + end_src as u32,
                    sl,
                    sc,
                    el,
                    ec,
                );
            }
            new_children.push(new_text_id);
        }

        let link_id = arena.alloc_node(MdastNodeType::Link as u8);
        let url_sr = arena.alloc_string(&url_for_node);
        let link_data = LinkData {
            url: url_sr,
            title: StringRef::empty(),
        };
        arena.set_type_data(link_id, &link_data.to_bytes());
        if with_position {
            // Same chunk-source-position logic as the prior chunk: source
            // span starts where text[s] is represented (incl. any escapes).
            // Link start: use text[s]'s actual source position (map[s]), not
            // chunk_src_pos(s). chunk_src_pos extends back to include consumed
            // bytes (escapes, dropped indent on continuation lines) — those
            // belong to the preceding TEXT chunk's end span, not the link's
            // start. For an email `... \n<space>foo@bar`, the space at
            // source[s-1] is NOT part of the email's range.
            let link_start_fallback = || -> usize {
                if let Some(map) = text_to_source.as_ref() {
                    if s < map.len() {
                        map[s]
                    } else {
                        chunk_src_pos(s)
                    }
                } else {
                    chunk_src_pos(s)
                }
            };
            let s_src = span_offsets
                .map(|(a, _)| a)
                .unwrap_or_else(link_start_fallback);
            let e_src = span_offsets
                .map(|(_, b)| b)
                .unwrap_or_else(|| chunk_src_pos(e));
            let (sl, sc) = line_col_at_src(s_src);
            let (el, ec) = line_col_at_src(e_src);
            arena.set_position(
                link_id,
                start_offset + s_src as u32,
                start_offset + e_src as u32,
                sl,
                sc,
                el,
                ec,
            );
        }
        let link_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let disp_sr = arena.alloc_string(displayed);
        arena.set_type_data(link_text_id, &disp_sr.as_bytes());
        if with_position {
            // Link start: use text[s]'s actual source position (map[s]), not
            // chunk_src_pos(s). chunk_src_pos extends back to include consumed
            // bytes (escapes, dropped indent on continuation lines) — those
            // belong to the preceding TEXT chunk's end span, not the link's
            // start. For an email `... \n<space>foo@bar`, the space at
            // source[s-1] is NOT part of the email's range.
            let link_start_fallback = || -> usize {
                if let Some(map) = text_to_source.as_ref() {
                    if s < map.len() {
                        map[s]
                    } else {
                        chunk_src_pos(s)
                    }
                } else {
                    chunk_src_pos(s)
                }
            };
            let s_src = span_offsets
                .map(|(a, _)| a)
                .unwrap_or_else(link_start_fallback);
            let e_src = span_offsets
                .map(|(_, b)| b)
                .unwrap_or_else(|| chunk_src_pos(e));
            let (sl, sc) = line_col_at_src(s_src);
            let (el, ec) = line_col_at_src(e_src);
            arena.set_position(
                link_text_id,
                start_offset + s_src as u32,
                start_offset + e_src as u32,
                sl,
                sc,
                el,
                ec,
            );
        }
        arena.set_children(link_id, &[link_text_id]);
        new_children.push(link_id);

        // Emit the URL trail (bytes between the trimmed URL end and the
        // raw end of the URL-shaped span) as its OWN text node when:
        //  - the URL was suppressed by an unbalanced `[` AND the trail
        //    needs to stay literal alongside the original `[`, OR
        //  - the post-trail content starts on a new line — micromark
        //    emits the trail as a separate text event, and mdast splits
        //    text nodes at line boundaries, so a trail-then-newline
        //    (e.g. `https://../>\nfoo`) produces TWO text nodes.
        //  - the trail sits at end of text with nothing following
        //    (still a separate node, since position attaches to the
        //    trail span itself).
        //
        // Otherwise (same-line content follows the trail like `) for x`),
        // mdast merges trail + post-trail into one text node — emit the
        // trail as part of the trailing chunk by leaving cursor at `e`.
        let split_trail = raw_e > e
            && (preceded_by_open_bracket[idx]
                || raw_e >= bytes.len()
                || matches!(bytes.get(raw_e), Some(b'\n' | b'\r')));
        if split_trail {
            let chunk = &text[e..raw_e];
            let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
            let chunk_sr = arena.alloc_string(chunk);
            arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
            // Trail position emission mirrors the link's: when the link
            // had a position (construct path accepted), emit a position
            // for the trail too. find-and-replace doesn't carry positions,
            // so leave position-less in that case.
            if with_position {
                let e_src = chunk_src_pos(e);
                let raw_e_src = chunk_src_pos(raw_e);
                let (sl, sc) = line_col_at_src(e_src);
                let (el, ec) = line_col_at_src(raw_e_src);
                arena.set_position(
                    new_text_id,
                    start_offset + e_src as u32,
                    start_offset + raw_e_src as u32,
                    sl,
                    sc,
                    el,
                    ec,
                );
            }
            new_children.push(new_text_id);
            cursor = raw_e;
        } else {
            cursor = e;
        }
    }

    if cursor < bytes.len() {
        let chunk = &text[cursor..];
        let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let chunk_sr = arena.alloc_string(chunk);
        arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
        // The trailing chunk after the last autolink: emit with position if
        // every replacement above was position-emitting (the whole text node
        // was clean of unbalanced brackets) and no URL was rewritten,
        // else leave it position-less.
        if !preceded_by_open_bracket.iter().any(|x| *x) && !any_url_rewritten {
            let cursor_src = chunk_src_pos(cursor);
            let end_src = chunk_src_pos(bytes.len());
            let (sl, sc) = line_col_at_src(cursor_src);
            let (el, ec) = line_col_at_src(end_src);
            arena.set_position(
                new_text_id,
                start_offset + cursor_src as u32,
                start_offset + end_src as u32,
                sl,
                sc,
                el,
                ec,
            );
        }
        new_children.push(new_text_id);
    }

    arena.replace_node_with_children(text_id, &new_children);
}

/// Append a text value as an MDAST Text leaf, merging with the previous
/// sibling text node when possible. Matches the behavior remark inherits
/// from `mdast-util-from-markdown`, which coalesces adjacent text nodes
/// that result from entity decoding, character synthesis, etc.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_text_merging(
    builder: &mut ArenaBuilder<Mdast>,
    text_value: &str,
    start: u32,
    end: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
) {
    if let Some(pid) = builder.last_sibling_id() {
        let prev = builder.arena_ref().get_node(pid);
        if prev.node_type == MdastNodeType::Text as u8 {
            let prev_data = builder.arena_ref().get_type_data(pid);
            if prev_data.len() >= 8 {
                let prev_sr = StringRef::from_bytes(prev_data);
                let prev_text = builder.arena_ref().get_str(prev_sr);
                let combined = [prev_text, text_value].concat();
                let new_sr = builder.alloc_string(&combined);
                let pn = builder.arena_ref().get_node(pid);
                builder.update_leaf_full(
                    pid,
                    pn.start_offset,
                    end,
                    pn.start_line,
                    pn.start_column,
                    end_line,
                    end_col,
                    &new_sr.as_bytes(),
                );
                return;
            }
        }
    }
    let sr = builder.alloc_string(text_value);
    builder.add_leaf_full(
        MdastNodeType::Text as u8,
        start,
        end,
        start_line,
        start_col,
        end_line,
        end_col,
        &sr.as_bytes(),
    );
}

/// For each `Text` node that lives directly under a directive's label, scan
/// for balanced backtick runs and split the text into `text + inlineCode + text`
/// pieces. This matches the common `:::tip[Set a \`baseUrl\`]` pattern without
/// needing to re-run the full inline parser on the label substring.
pub(crate) fn directive_label_inline_code_pass(arena: &mut Arena<Mdast>) {
    // Collect candidate text node ids first (pair: parent id, text id).
    let mut candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        // Text value must contain a backtick to be worth processing.
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('`') {
            continue;
        }

        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);

        let is_directive_label = match parent_type {
            // Text directly under a leaf/text directive — the directive's
            // children ARE the label.
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            // Paragraph under a container directive is the label iff it has
            // the `directiveLabel:true` marker.
            Some(MdastNodeType::Paragraph) => {
                let node_data = arena.get_node_data(parent_id);
                node_data
                    .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                    .unwrap_or(false)
            }
            _ => false,
        };
        if !is_directive_label {
            continue;
        }
        candidates.push(id);
    }

    for text_id in candidates {
        split_text_on_backticks(arena, text_id);
    }
}

/// Split a `Text` node's value into `text + inlineCode + text …` on balanced
/// backtick runs. Only handles the simple case (same-length opening/closing
/// runs, single-line), which is what directive labels carry in practice.
fn split_text_on_backticks(arena: &mut Arena<Mdast>, text_id: u32) {
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    // Fast-path: no backtick anywhere → nothing to split, skip the clone.
    if memchr::memchr(b'`', arena.get_str(sr).as_bytes()).is_none() {
        return;
    }
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    // Find all balanced backtick pairs.
    #[derive(Clone, Copy)]
    struct Pair {
        open_start: usize,
        open_end: usize,
        close_start: usize,
        close_end: usize,
    }
    let mut pairs: Vec<Pair> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'`' {
            i += 1;
            continue;
        }
        // Count run length.
        let open_start = i;
        while i < bytes.len() && bytes[i] == b'`' {
            i += 1;
        }
        let open_end = i;
        let run_len = open_end - open_start;
        // Find matching closing run of the same length.
        let mut j = i;
        let matched_close: Option<(usize, usize)> = loop {
            if j >= bytes.len() {
                break None;
            }
            if bytes[j] == b'`' {
                let close_start = j;
                while j < bytes.len() && bytes[j] == b'`' {
                    j += 1;
                }
                let close_end = j;
                if close_end - close_start == run_len {
                    break Some((close_start, close_end));
                }
                // Not a match; skip this run and continue searching.
                continue;
            }
            j += 1;
        };
        if let Some((cs, ce)) = matched_close {
            pairs.push(Pair {
                open_start,
                open_end,
                close_start: cs,
                close_end: ce,
            });
            i = ce;
        }
    }

    if pairs.is_empty() {
        return;
    }

    // Build the replacement child list.
    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for p in pairs {
        // Leading plain text.
        if p.open_start > cursor {
            let segment = &text[cursor..p.open_start];
            if !segment.is_empty() {
                let seg_sr = arena.alloc_string(segment);
                let tid = arena.alloc_node(MdastNodeType::Text as u8);
                arena.set_type_data(tid, &seg_sr.as_bytes());
                arena.set_position(
                    tid,
                    base_start + cursor as u32,
                    base_start + p.open_start as u32,
                    base_line,
                    base_col + cursor as u32,
                    base_line,
                    base_col + p.open_start as u32,
                );
                new_children.push(tid);
            }
        }
        // Inline code.
        let code_value = &text[p.open_end..p.close_start];
        let code_sr = arena.alloc_string(code_value);
        let cid = arena.alloc_node(MdastNodeType::InlineCode as u8);
        arena.set_type_data(cid, &code_sr.as_bytes());
        arena.set_position(
            cid,
            base_start + p.open_start as u32,
            base_start + p.close_end as u32,
            base_line,
            base_col + p.open_start as u32,
            base_line,
            base_col + p.close_end as u32,
        );
        new_children.push(cid);
        cursor = p.close_end;
    }
    // Trailing plain text.
    if cursor < text.len() {
        let segment = &text[cursor..];
        let seg_sr = arena.alloc_string(segment);
        let tid = arena.alloc_node(MdastNodeType::Text as u8);
        arena.set_type_data(tid, &seg_sr.as_bytes());
        arena.set_position(
            tid,
            base_start + cursor as u32,
            base_start + text.len() as u32,
            base_line,
            base_col + cursor as u32,
            base_line,
            base_col + text.len() as u32,
        );
        new_children.push(tid);
    }

    arena.replace_node_with_children(text_id, &new_children);
}

/// Post-pass matching `directive_label_inline_code_pass` for JSX tags. For
/// each `Text` node directly under a directive label, split on balanced
/// `<Name>…</Name>` (or self-closing `<Name/>`) runs and emit
/// `mdxJsxTextElement` children. Also splits on balanced `{…}` spans and
/// emits `mdxTextExpression` nodes.
pub(crate) fn directive_label_jsx_pass(arena: &mut Arena<Mdast>) {
    let mut candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('<') && !text.contains('{') {
            continue;
        }
        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);
        let is_directive_label = match parent_type {
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            Some(MdastNodeType::Paragraph) => arena
                .get_node_data(parent_id)
                .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                .unwrap_or(false),
            _ => false,
        };
        if !is_directive_label {
            continue;
        }
        candidates.push(id);
    }
    for text_id in candidates {
        split_text_on_jsx_tags(arena, text_id);
    }
    // Second pass picks up text nodes created by the first split and emits
    // MDX text expressions for `{…}` runs.
    let mut expr_candidates: Vec<u32> = Vec::new();
    for id in 0..arena.len() as u32 {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Text as u8 {
            continue;
        }
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        if !text.contains('{') {
            continue;
        }
        let parent_id = node.parent;
        let parent = arena.get_node(parent_id);
        let parent_type = MdastNodeType::from_u8(parent.node_type);
        let in_label = match parent_type {
            Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective) => true,
            Some(MdastNodeType::Paragraph) => arena
                .get_node_data(parent_id)
                .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                .unwrap_or(false),
            // Also handle the children of a JSX text element created by the
            // first pass — they also live under a directive label.
            Some(MdastNodeType::MdxJsxTextElement) => {
                let grandparent_id = parent.parent;
                if grandparent_id == u32::MAX {
                    false
                } else {
                    let grandparent = arena.get_node(grandparent_id);
                    let gp_type = MdastNodeType::from_u8(grandparent.node_type);
                    matches!(
                        gp_type,
                        Some(MdastNodeType::LeafDirective | MdastNodeType::TextDirective)
                    ) || (gp_type == Some(MdastNodeType::Paragraph)
                        && arena
                            .get_node_data(grandparent_id)
                            .map(|d| d.starts_with(b"{\"directiveLabel\":true}"))
                            .unwrap_or(false))
                }
            }
            _ => false,
        };
        if !in_label {
            continue;
        }
        expr_candidates.push(id);
    }
    for text_id in expr_candidates {
        split_text_on_mdx_expressions(arena, text_id);
    }
}

/// Split a `Text` node on `{…}` spans (balanced braces, JS-aware) and emit
/// `mdxTextExpression` nodes for the matched spans.
fn split_text_on_mdx_expressions(arena: &mut Arena<Mdast>, text_id: u32) {
    use crate::mdx::scan_mdx_inline_expression;
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    // Fast-path: no `{` anywhere → no expression spans possible.
    if memchr::memchr(b'{', arena.get_str(sr).as_bytes()).is_none() {
        return;
    }
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();
    let mut spans: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let Some((content_start, content_end, total_len)) = scan_mdx_inline_expression(&bytes[i..])
        else {
            i += 1;
            continue;
        };
        spans.push((i, i + total_len, i + content_start, i + content_end));
        i += total_len;
    }
    if spans.is_empty() {
        return;
    }
    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for (span_start, span_end, content_start, content_end) in spans {
        if span_start > cursor {
            let seg = &text[cursor..span_start];
            let seg_sr = arena.alloc_string(seg);
            let tid = arena.alloc_node(MdastNodeType::Text as u8);
            arena.set_type_data(tid, &seg_sr.as_bytes());
            arena.set_position(
                tid,
                base_start + cursor as u32,
                base_start + span_start as u32,
                base_line,
                base_col + cursor as u32,
                base_line,
                base_col + span_start as u32,
            );
            new_children.push(tid);
        }
        let content = &text[content_start..content_end];
        let content_sr = arena.alloc_string(content);
        let eid = arena.alloc_node(MdastNodeType::MdxTextExpression as u8);
        arena.set_type_data(eid, &content_sr.as_bytes());
        arena.set_position(
            eid,
            base_start + span_start as u32,
            base_start + span_end as u32,
            base_line,
            base_col + span_start as u32,
            base_line,
            base_col + span_end as u32,
        );
        new_children.push(eid);
        cursor = span_end;
    }
    if cursor < text.len() {
        let seg = &text[cursor..];
        let seg_sr = arena.alloc_string(seg);
        let tid = arena.alloc_node(MdastNodeType::Text as u8);
        arena.set_type_data(tid, &seg_sr.as_bytes());
        arena.set_position(
            tid,
            base_start + cursor as u32,
            base_start + text.len() as u32,
            base_line,
            base_col + cursor as u32,
            base_line,
            base_col + text.len() as u32,
        );
        new_children.push(tid);
    }
    arena.replace_node_with_children(text_id, &new_children);
}

/// Split a `Text` node on `<Name>…</Name>` / `<Name/>` spans, producing
/// `mdxJsxTextElement` nodes for the matched spans. The inner content of a
/// matched open/close pair becomes a child `Text` node (no recursion — nested
/// JSX inside a directive label is rare enough that a single-level split
/// covers the conformance cases).
fn split_text_on_jsx_tags(arena: &mut Arena<Mdast>, text_id: u32) {
    use crate::mdx::{parse_jsx_tag, scan_mdx_inline_jsx};
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    // Fast-path: no `<` anywhere → no JSX tag spans possible.
    if memchr::memchr(b'<', arena.get_str(sr).as_bytes()).is_none() {
        return;
    }
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

    #[derive(Clone)]
    enum Span {
        SelfClosing {
            start: usize,
            end: usize,
            name: alloc::string::String,
        },
        Paired {
            start: usize,
            open_end: usize,
            close_start: usize,
            end: usize,
            name: alloc::string::String,
        },
    }

    let mut spans: Vec<Span> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let Some(tag_end) = scan_mdx_inline_jsx(&bytes[i..]) else {
            i += 1;
            continue;
        };
        let tag_raw = &text[i..i + tag_end];
        let jsx = parse_jsx_tag(tag_raw);
        if jsx.is_closing {
            i += tag_end;
            continue;
        }
        if jsx.is_self_closing {
            spans.push(Span::SelfClosing {
                start: i,
                end: i + tag_end,
                name: jsx.name.to_string(),
            });
            i += tag_end;
            continue;
        }
        // Opening tag — scan forward for a matching `</name>`.
        let name = jsx.name.to_string();
        let open_end = i + tag_end;
        let mut j = open_end;
        let mut close_span: Option<(usize, usize)> = None;
        while j < bytes.len() {
            if bytes[j] != b'<' {
                j += 1;
                continue;
            }
            let Some(inner_tag_end) = scan_mdx_inline_jsx(&bytes[j..]) else {
                j += 1;
                continue;
            };
            let inner_tag = &text[j..j + inner_tag_end];
            let inner_jsx = parse_jsx_tag(inner_tag);
            if inner_jsx.is_closing && inner_jsx.name.as_ref() == name.as_str() {
                close_span = Some((j, j + inner_tag_end));
                break;
            }
            j += inner_tag_end;
        }
        if let Some((close_start, close_end)) = close_span {
            spans.push(Span::Paired {
                start: i,
                open_end,
                close_start,
                end: close_end,
                name,
            });
            i = close_end;
        } else {
            i = open_end;
        }
    }

    if spans.is_empty() {
        return;
    }

    let node = arena.get_node(text_id);
    let base_start = node.start_offset;
    let base_line = node.start_line;
    let base_col = node.start_column;

    let push_text = |arena: &mut Arena<Mdast>,
                     out: &mut Vec<u32>,
                     segment: &str,
                     seg_start: usize,
                     seg_end: usize| {
        if segment.is_empty() {
            return;
        }
        let seg_sr = arena.alloc_string(segment);
        let tid = arena.alloc_node(MdastNodeType::Text as u8);
        arena.set_type_data(tid, &seg_sr.as_bytes());
        arena.set_position(
            tid,
            base_start + seg_start as u32,
            base_start + seg_end as u32,
            base_line,
            base_col + seg_start as u32,
            base_line,
            base_col + seg_end as u32,
        );
        out.push(tid);
    };

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for span in spans {
        match span {
            Span::SelfClosing { start, end, name } => {
                push_text(
                    arena,
                    &mut new_children,
                    &text[cursor..start],
                    cursor,
                    start,
                );
                let name_sr = arena.alloc_string(&name);
                let jsx_data = satteri_ast::mdast::encode_mdx_jsx_element_data(name_sr, &[], true);
                let jid = arena.alloc_node(MdastNodeType::MdxJsxTextElement as u8);
                arena.set_type_data(jid, &jsx_data);
                arena.set_node_data(jid, MDX_EXPLICIT_JSX_DATA.to_vec());
                arena.set_position(
                    jid,
                    base_start + start as u32,
                    base_start + end as u32,
                    base_line,
                    base_col + start as u32,
                    base_line,
                    base_col + end as u32,
                );
                new_children.push(jid);
                cursor = end;
            }
            Span::Paired {
                start,
                open_end,
                close_start,
                end,
                name,
            } => {
                push_text(
                    arena,
                    &mut new_children,
                    &text[cursor..start],
                    cursor,
                    start,
                );
                let name_sr = arena.alloc_string(&name);
                let jsx_data = satteri_ast::mdast::encode_mdx_jsx_element_data(name_sr, &[], true);
                let jid = arena.alloc_node(MdastNodeType::MdxJsxTextElement as u8);
                arena.set_type_data(jid, &jsx_data);
                arena.set_node_data(jid, MDX_EXPLICIT_JSX_DATA.to_vec());
                arena.set_position(
                    jid,
                    base_start + start as u32,
                    base_start + end as u32,
                    base_line,
                    base_col + start as u32,
                    base_line,
                    base_col + end as u32,
                );
                // Inner text child.
                let inner = &text[open_end..close_start];
                if !inner.is_empty() {
                    let inner_sr = arena.alloc_string(inner);
                    let cid = arena.alloc_node(MdastNodeType::Text as u8);
                    arena.set_type_data(cid, &inner_sr.as_bytes());
                    arena.set_position(
                        cid,
                        base_start + open_end as u32,
                        base_start + close_start as u32,
                        base_line,
                        base_col + open_end as u32,
                        base_line,
                        base_col + close_start as u32,
                    );
                    arena.set_children(jid, &[cid]);
                }
                new_children.push(jid);
                cursor = end;
            }
        }
    }
    push_text(
        arena,
        &mut new_children,
        &text[cursor..],
        cursor,
        text.len(),
    );

    arena.replace_node_with_children(text_id, &new_children);
}

pub(crate) fn mdx_mark_and_unravel(arena: &mut Arena<Mdast>) {
    let len = arena.len() as u32;
    // Only paragraphs containing inline MDX nodes can be promoted; without
    // any in the arena the per-paragraph work below is guaranteed wasted.
    let has_inline_mdx = (0..len).any(|id| {
        matches!(
            MdastNodeType::from_u8(arena.get_node(id).node_type),
            Some(MdastNodeType::MdxJsxTextElement | MdastNodeType::MdxTextExpression),
        )
    });
    if !has_inline_mdx {
        return;
    }
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != MdastNodeType::Paragraph as u8 {
            continue;
        }
        let children = arena.get_children(id).to_vec();
        if children.is_empty() {
            continue;
        }
        let mut all_mdx = true;
        let mut has_mdx = false;
        for &child_id in &children {
            let child = arena.get_node(child_id);
            match MdastNodeType::from_u8(child.node_type) {
                Some(MdastNodeType::MdxJsxTextElement | MdastNodeType::MdxTextExpression) => {
                    has_mdx = true;
                }
                Some(MdastNodeType::Text) => {
                    let data = arena.get_type_data(child_id);
                    if !data.is_empty() {
                        let sr = decode_string_ref_data(data);
                        let text = arena.get_str(sr);
                        if !text.chars().all(|c| c.is_ascii_whitespace()) {
                            all_mdx = false;
                            break;
                        }
                    }
                }
                _ => {
                    all_mdx = false;
                    break;
                }
            }
        }
        if !all_mdx || !has_mdx {
            continue;
        }
        let mut promoted: Vec<u32> = Vec::new();
        for &child_id in &children {
            let child = arena.get_node(child_id);
            match MdastNodeType::from_u8(child.node_type) {
                Some(MdastNodeType::MdxJsxTextElement) => {
                    arena.get_node_mut(child_id).node_type = MdastNodeType::MdxJsxFlowElement as u8;
                    promoted.push(child_id);
                }
                Some(MdastNodeType::MdxTextExpression) => {
                    arena.get_node_mut(child_id).node_type = MdastNodeType::MdxFlowExpression as u8;
                    promoted.push(child_id);
                }
                Some(MdastNodeType::Text) => {
                    let data = arena.get_type_data(child_id);
                    if !data.is_empty() {
                        let sr = decode_string_ref_data(data);
                        let text = arena.get_str(sr);
                        if !text.chars().all(|c| c.is_ascii_whitespace()) {
                            promoted.push(child_id);
                        }
                    }
                }
                _ => {
                    promoted.push(child_id);
                }
            }
        }
        arena.replace_node_with_children(id, &promoted);
    }
}
