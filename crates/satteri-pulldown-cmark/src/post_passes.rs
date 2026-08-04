//! Post-passes that transform the built MDAST tree.
//!
//! `arena_build::parse` produces a structurally complete `Arena<Mdast>`
//! that matches micromark's tokenizer output. The remark ecosystem then
//! layers several `mdast-util-*` / `remark-*` plugins on top to:
//!
//! * recognize bare URLs and emails inside text nodes
//!   ([`gfm_autolink_literal_pass`]),
//! * mark and unravel MDX-only flow children
//!   ([`mdx_mark_and_unravel`]).
//!
//! Directive labels used to be inline-parsed here too; that now happens in the
//! parser (firstpass `DirectiveLabel` and the leaf/text directive children).
//!
//! Each of those is a self-contained tree-walking transformation that
//! reads / mutates `Arena<Mdast>` after building is finished. They live
//! here so [`arena_build`] stays focused on actually building the arena.

#[cfg(feature = "mdx")]
use satteri_arena::decode_string_ref_data;
use satteri_arena::{Arena, ArenaBuilder, Mdast, StringRef};
use satteri_ast::mdast::{codec::LinkData, MdastNodeType};

use crate::puncttable::is_punctuation;

#[cfg(feature = "mdx")]
pub(crate) const MDX_EXPLICIT_JSX_DATA: &[u8] = b"{\"_mdxExplicitJsx\":true}";

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

/// Match an autolink-literal scheme at `ix`, returning `(proto_len, is_www)`.
/// Case-insensitive: micromark's `protocolPrefixInside` lowercases the
/// scheme before comparing, and its `wwwPrefix` accepts `W` as well as `w`.
/// remark-gfm handles `http(s)` and `www.`, but not `ftp`.
pub(crate) fn match_autolink_scheme(bytes: &[u8], ix: usize) -> Option<(usize, bool)> {
    let rest = &bytes[ix..];
    let ci = |prefix: &[u8]| {
        rest.len() >= prefix.len() && rest[..prefix.len()].eq_ignore_ascii_case(prefix)
    };
    if ci(b"https://") {
        Some((8, false))
    } else if ci(b"http://") {
        Some((7, false))
    } else if ci(b"www.") {
        Some((4, true))
    } else {
        None
    }
}

/// True when `bytes[i..end]` is entirely "trail" per micromark's
/// `tokenizeTrail`: regular punctuation (`!"'*,.:;?_~`), `]`, `)`, or whole
/// `&[a-zA-Z]+;` entities. Such a run is not part of the link, so when it
/// reaches the body boundary (`end`, always whitespace/`<`/EOF) the link
/// ends where the run starts.
fn trail_is_all(bytes: &[u8], mut i: usize, end: usize) -> bool {
    while i < end {
        match bytes[i] {
            b'!' | b'"' | b'\'' | b')' | b'*' | b',' | b'.' | b':' | b';' | b'?' | b']' | b'_'
            | b'~' => i += 1,
            // `&[a-zA-Z]+;` (micromark's `trailCharacterReference`).
            b'&' => {
                let mut j = i + 1;
                while j < end && bytes[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if j > i + 1 && j < end && bytes[j] == b';' {
                    i = j + 1;
                } else {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Find where a `http(s)`/`www` URL body ends, replicating micromark's
/// `path`/`domain` + `trail` tokenizers. Walks forward from `start` tracking
/// parenthesis balance: a `)` that closes an earlier `(` stays in the link,
/// but any trailing-punctuation marker (including a now-unbalanced `)`)
/// followed only by trail ends the link. This forward scan (rather than a
/// right-to-left trim) is what gets `(b.)` right: once a trail starts at the
/// `.`, the balanced `)` is part of the trail and trimmed too.
fn construct_url_end(bytes: &[u8], start: usize, raw_end: usize) -> usize {
    let (mut size_open, mut size_close) = (0usize, 0usize);
    let mut i = start;
    while i < raw_end {
        let b = bytes[i];
        if b == b'(' {
            size_open += 1;
        } else if b == b')' && size_close < size_open {
            size_close += 1;
        } else if matches!(
            b,
            b'!' | b'"'
                | b'&'
                | b'\''
                | b')'
                | b'*'
                | b','
                | b'.'
                | b':'
                | b';'
                | b'?'
                | b']'
                | b'_'
                | b'~'
        ) {
            if trail_is_all(bytes, i, raw_end) {
                return i;
            }
            if b == b')' {
                size_close += 1;
            }
        }
        i += 1;
    }
    raw_end
}

pub(crate) fn scan_autolink_literal(
    bytes: &[u8],
    ix: usize,
    prev_is_content_start: bool,
) -> Option<(usize, usize, usize, String, bool)> {
    let (proto_len, is_www) = match_autolink_scheme(bytes, ix)?;

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
    // When the trigger sits at the start of the inline content (right after a
    // container marker like `>`/`-`, which the inline stream doesn't see),
    // micromark's `self.previous` is a line ending, so both preceding-char
    // rules pass, exactly as at the start of the document.
    let prev_loose_only = if ix > 0 && !prev_is_content_start {
        let prev = bytes[ix - 1];
        // The construct's preceding-char rule differs by scheme:
        //   * www: `previousWww`, a fixed set (`(`, `*`, `_`, `[`, `]`, `~`,
        //     line ending or space). Nothing else, so `5www.x` doesn't fire.
        //   * http(s): `previousProtocol` rejects only ASCII alphabetic;
        //     digits, punctuation, and any non-ASCII byte (Cyrillic etc.)
        //     pass, so the construct can fire after `п` in `_oпhttps://...`.
        let prev_loose_ok = if is_www {
            matches!(
                prev,
                b'(' | b'*' | b'_' | b'[' | b']' | b'~' | b' ' | b'\t' | b'\r' | b'\n'
            )
        } else if prev < 0x80 {
            !prev.is_ascii_alphabetic()
        } else {
            true
        };
        if !prev_loose_ok {
            return None;
        }
        !fnr_previous_ok(bytes, ix)
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
        if !b.is_ascii_graphic() {
            // One range test leaves the hot path: control, space, DEL, or a
            // scalar that has to be decoded before it can be judged.
            if b < 0x80 || char_at(bytes, end).is_some_and(is_autolink_whitespace) {
                break;
            }
        } else if b == b'<' {
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

    // Need at least one byte past the scheme — except a bare `www.` followed
    // by a non-EOF byte, which links just `www`: micromark's `wwwPrefix`
    // succeeds for any non-EOF char after the dot, the `www` letters are the
    // domain, and the `.` is trail. So `www. rest` links `www` (construct
    // path, trail merged with ` rest`), but `www.` at true EOF falls to FNR.
    if end == ix + proto_len && !(is_www && ix + proto_len < bytes.len()) {
        return None;
    }

    let raw_end = end;

    // Trim trailing punctuation via micromark's forward `path`/`trail` scan.
    // For `www`, start the scan at the `.` (one byte into the prefix) so a
    // trailing dot can itself be trimmed (`www.!"~` links bare `www`), since
    // the `www` letters are the domain and everything after `.` is trail.
    let scan_start = if is_www { ix + 3 } else { ix + proto_len };
    end = construct_url_end(bytes, scan_start, raw_end);

    // The kept URL must be non-empty past the scheme: for http(s) that's a
    // byte after `://`; for `www` the `www` letters always remain.
    if end <= if is_www { ix } else { ix + proto_len } {
        return None;
    }

    // micromark's `tokenizeDomain`/`domainAfter` requires only a non-empty
    // domain (`seen`) with no trailing `_`; a dot is *not* required. So
    // `www.localhost` and `http://localhost` both autolink. (See the GH
    // #279 note in micromark's `domainAfter`.) For `www`, that tokenizer
    // consumes the `www.` prefix as part of the domain, so its `w`s satisfy
    // `seen` even when nothing alphanumeric follows (`www..%&`); include the
    // prefix here. http(s) domains start after `://`.
    let body = if is_www {
        &bytes[ix..end]
    } else {
        &bytes[ix + proto_len..end]
    };

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
/// `dot_needs_alnum` selects the domain rule, which differs between the two
/// pipelines: the construct's `emailDomainDotTrail` keeps a `.` only when an
/// *alphanumeric* follows (so `a@b._c` stops at `b`), while the FNR regex's
/// `(?:\.[-\w]+)+` also accepts `-`/`_` after the dot (so `a@b._c` keeps
/// `b._c`). Construct callers pass `true`, the FNR caller `false`.
pub(crate) fn scan_email_autolink(
    bytes: &[u8],
    at_ix: usize,
    dot_needs_alnum: bool,
) -> Option<(usize, usize, String, bool)> {
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
    // Domain per micromark's `emailDomain`: labels of alphanumeric, `-`, `_`,
    // joined by a `.` that is kept only when *followed by an alphanumeric*
    // (`emailDomainDotTrail`). So a `..`, a trailing `.`, or a `.` before
    // `-`/`_` ends the domain (`a@b.com...x` links only `a@b.com`), while a
    // literal leading `.` is allowed (`y@.bar.baz`). The FNR pipeline's
    // stricter "first domain char must be `[-\w]`" rule lives in
    // `fnr_find_email`, since this scanner also feeds the construct path.
    let mut end = at_ix + 1;
    while end < bytes.len() {
        let b = bytes[end];
        let is_label = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_');
        // A `.` is kept only when the next byte continues a label: an
        // alphanumeric for the construct, or any `[-\w]` for FNR.
        let after_dot_ok = bytes.get(end + 1).is_some_and(|&n| {
            n.is_ascii_alphanumeric() || (!dot_needs_alnum && matches!(n, b'-' | b'_'))
        });
        if is_label || (b == b'.' && after_dot_ok) {
            end += 1;
        } else {
            break;
        }
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
    // mdast-util-gfm-autolink-literal's `findEmail` only rejects when the
    // *last* character of the label is in `[-\d_]`. We already handle
    // that above. `_` elsewhere in the domain is permitted.
    let _ = tld;
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
                        | MdastNodeType::Superscript
                        | MdastNodeType::Subscript
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

/// Tree-level fallback for GFM autolink literals. The inline construct in
/// `firstpass.rs` handles the common case; this pass picks up URL/email
/// patterns that survived in plain Text nodes because the construct didn't
/// fire (preceded by a digit, inside a failed `<...>` autolink, across
/// container prefixes).
pub(crate) fn gfm_autolink_literal_pass(
    arena: &mut Arena<Mdast>,
    source_bytes: &[u8],
    options: crate::Options,
    mut cursor: Option<&mut satteri_arena::LineIndexCursor<'_, '_>>,
) {
    let len = arena.len() as u32;
    let mut candidates: Vec<u32> = Vec::new();
    let text_ty = MdastNodeType::Text as u8;
    for id in 0..len {
        let node = arena.get_node(id);
        if node.node_type != text_ty {
            continue;
        }
        if node.parent == u32::MAX || node.parent >= len {
            continue;
        }
        // `findAndReplace`'s `{ignore: ['link', 'linkReference']}` skips a
        // node *and its whole subtree*, so walk the ancestor chain: a `www.`
        // inside `[<del>www</del>](/x)` (parent `delete`, grandparent `link`)
        // must be skipped too. Image alt-text and code/expression/frontmatter
        // subtrees likewise never autolink.
        let mut ancestor = node.parent;
        let mut inside_ignored = false;
        while ancestor != u32::MAX && ancestor < len {
            if matches!(
                MdastNodeType::from_u8(arena.get_node(ancestor).node_type),
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
                inside_ignored = true;
                break;
            }
            ancestor = arena.get_node(ancestor).parent;
        }
        if inside_ignored {
            continue;
        }
        let data = arena.get_type_data(id);
        if data.is_empty() {
            continue;
        }
        let sr = StringRef::from_bytes(data);
        let text = arena.get_str(sr);
        let bytes = text.as_bytes();
        // Triggers are case-insensitive (`HTTP://`, `WWW.`), so scan for the
        // uppercase variants too.
        if bytes
            .iter()
            .any(|&b| matches!(b, b'h' | b'H' | b'w' | b'W' | b'@'))
        {
            candidates.push(id);
        }
    }
    let smart = Smart {
        quotes: options.has_smart_quotes(),
        dashes: options.has_smart_dashes(),
        ellipses: options.has_smart_ellipses(),
    };
    for node_id in candidates {
        split_text_with_autolinks_fnr(arena, node_id, source_bytes, cursor.as_deref_mut(), smart);
    }
}

/// The whitespace class autolinks use: `White_Space` less U+0085, plus U+FEFF.
#[inline]
fn is_autolink_whitespace(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{FEFF}'
}

/// The scalar starting at `ix`.
fn char_at(bytes: &[u8], ix: usize) -> Option<char> {
    let rest = bytes.get(ix..)?;
    let width = match *rest.first()? {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    };
    core::str::from_utf8(rest.get(..width)?)
        .ok()?
        .chars()
        .next()
}

/// The scalar before `ix`, or `None` at the start of the input.
fn preceding_char(bytes: &[u8], ix: usize) -> Option<char> {
    if ix == 0 {
        return None;
    }
    let prev = bytes[ix - 1];
    if prev < 0x80 {
        return Some(prev as char);
    }
    let mut start = ix - 1;
    while start > 0 && bytes[start] & 0xC0 == 0x80 {
        start -= 1;
    }
    core::str::from_utf8(&bytes[start..ix])
        .ok()?
        .chars()
        .next_back()
}

/// Boundary rule for the fallback pass. Stricter than the construct's, which
/// rejects only alphabetic, so digits and non-ASCII letters fail here.
///
/// Classifying the whole scalar accepts astral punctuation and symbols, a
/// deliberate divergence — see `divergences.md`.
pub(crate) fn fnr_previous_ok(bytes: &[u8], ix: usize) -> bool {
    match preceding_char(bytes, ix) {
        None => true,
        Some(c) => is_autolink_whitespace(c) || is_punctuation(c),
    }
}

/// FNR's `findUrl` equivalent. Mirrors the
/// `(https?:\/\/|www(?=\.))([-.\w]+)([^ \t\r\n]*)` regex + `previous()` +
/// `isCorrectDomain` + `splitUrl` validation chain from
/// `mdast-util-gfm-autolink-literal`.
///
/// Returns `(start, url_end, full_url, raw_end)` where `url_end..raw_end`
/// is the splitUrl trail (kept as its own text node by `findAndReplace`).
fn fnr_find_url(bytes: &[u8], ix: usize) -> Option<(usize, usize, String, usize)> {
    let (proto_len, is_www) = match_autolink_scheme(bytes, ix)?;
    let s = ix;
    if !fnr_previous_ok(bytes, s) {
        return None;
    }
    // Regex group 2, the domain, is `[-.\w]+` (alphanumeric, `.`, `_`, `-`).
    // For `www` the capture group starts at the `.` (group 1 is just `www`,
    // the dot is lookahead), so the scan begins one byte earlier than the
    // scheme length: a non-ASCII domain like `www.點看.com` leaves the dot in
    // group 2 and the rest in group 3 (the path), matching the regex.
    let domain_start = if is_www { s + 3 } else { s + proto_len };
    let mut p = domain_start;
    while p < bytes.len() {
        let b = bytes[p];
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
            p += 1;
        } else {
            break;
        }
    }
    let domain_end = p;
    if domain_end == domain_start {
        return None;
    }
    // Path class `[^ \t\r\n]*` (anything except markdown line ending/space).
    while p < bytes.len() {
        if matches!(bytes[p], b' ' | b'\t' | b'\r' | b'\n') {
            break;
        }
        p += 1;
    }
    let raw_end = p;
    // `isCorrectDomain`: ≥2 dot parts, no `_` in last/penult, alphanumeric
    // in non-empty parts. For `www.`, findUrl folds the `www` prefix into
    // the domain (`domain = protocol + domain`), so check from `s`; else
    // `www.localhost` would split to the single part `localhost` and fail.
    let domain_check_start = if is_www { s } else { domain_start };
    if !is_correct_domain_for_fnr(&bytes[domain_check_start..domain_end]) {
        return None;
    }
    // `splitUrl` trim — wider than the construct's trim set; includes
    // `>`, `}`, `&` (which the construct keeps) and excludes `*`, `_`,
    // `~` (which the construct trims).
    let url_end = split_url_trim_end(bytes, domain_start, raw_end);
    // mdast rejects when `splitUrl`'s kept part is empty. For http(s) that
    // part is the domain after `://`; for www it also includes the `www`
    // scheme letters (never trail), so `www.` alone still links as
    // `http://www`. Guard against the empty case per scheme.
    let min_nonempty = if is_www { s } else { domain_start };
    if url_end <= min_nonempty {
        return None;
    }
    let url_str = core::str::from_utf8(&bytes[s..url_end]).ok()?;
    let full_url = if is_www {
        format!("http://{url_str}")
    } else {
        url_str.to_string()
    };
    Some((s, url_end, full_url, raw_end))
}

/// FNR's `findEmail` equivalent. Mirrors the
/// `(?<=^|\s|\p{P}|\p{S})([-.\w+]+)@([-\w]+(?:\.[-\w]+)+)` regex + the
/// `previous(_, email=true)` + `/[-\d_]$/` rejection.
///
/// Returns `(start, end, "mailto:<addr>", raw_end)`. For emails the regex
/// has no trail, so `raw_end == end`. Uses `scan_email_autolink`'s walkback
/// (which retries from a shorter start when the max walkback's prev is
/// `/` or alphanumeric, matching FNR's `previous(_, true)` semantics).
fn fnr_find_email(bytes: &[u8], ix: usize) -> Option<(usize, usize, String, usize)> {
    let (mut s, e, _url, _retry) = scan_email_autolink(bytes, ix, false)?;
    // The regex's domain class is `[-\w]+(?:\.[-\w]+)+`. The first domain
    // char must be `[-\w]` (alphanumeric, `-`, `_`); `.` is rejected.
    let first_domain = *bytes.get(ix + 1)?;
    if !(first_domain.is_ascii_alphanumeric() || first_domain == b'-' || first_domain == b'_') {
        return None;
    }
    // FNR lookbehind `(?<=^|\s|\p{P}|\p{S})` (Unicode-aware): the regex anchors
    // the local part at the *first* boundary, but `scan_email_autolink`'s
    // walkback maximises it. When the maximal start is preceded by a letter
    // (ASCII or non-ASCII, e.g. `é`), advance to the next boundary: `é_.a@x`
    // links `.a@x`, not `_.a@x`. If no boundary precedes the `@`, there's no
    // match (this is also what keeps `пo\+@…`, whose `+` came from a source
    // escape that blocks the construct, from linking).
    while s < ix && !fnr_previous_ok(bytes, s) {
        s += 1;
    }
    if s >= ix {
        return None;
    }
    let addr = core::str::from_utf8(&bytes[s..e]).ok()?;
    Some((s, e, format!("mailto:{addr}"), e))
}

/// One aligned run between a Text node's decoded value and its raw source.
///
/// A 1:1 run splits at any interior offset; anything else (character
/// reference, escape, CRLF, stripped prefix) is atomic, and its interior
/// names no raw offset.
#[derive(Clone, Copy)]
struct Seg {
    d_start: u32,
    d_len: u32,
    r_start: u32,
    r_len: u32,
}

impl Seg {
    #[inline]
    fn is_one_to_one(&self) -> bool {
        self.d_len == self.r_len && self.d_len > 0
    }
}

/// Maps offsets in a Text node's decoded value back to the raw source the
/// value was built from. `Segments` is ascending and gapless in decoded space.
enum RawMap {
    /// The decoded value is byte-identical to its raw span. No allocation.
    Identity {
        r_start: u32,
    },
    Segments(Vec<Seg>),
}

impl RawMap {
    /// The segment producing decoded byte `d`; zero-width segments never match.
    fn seg_containing(segs: &[Seg], d: usize) -> Option<&Seg> {
        segs.iter()
            .find(|s| (s.d_start as usize) <= d && d < (s.d_start + s.d_len) as usize)
    }

    /// Raw offset where the decoded byte at `d` starts being produced.
    fn raw_start_of(&self, d: usize) -> Option<usize> {
        match self {
            RawMap::Identity { r_start } => Some(*r_start as usize + d),
            RawMap::Segments(segs) => match Self::seg_containing(segs, d) {
                Some(s) if s.is_one_to_one() => {
                    Some((s.r_start as usize) + (d - s.d_start as usize))
                }
                // An atomic run is included whole or not at all.
                Some(s) if d == s.d_start as usize => Some(s.r_start as usize),
                Some(_) => None,
                // Past the last decoded byte: the end of the raw span.
                None => segs.last().map(|s| (s.r_start + s.r_len) as usize),
            },
        }
    }

    /// Raw offset just past the production of the decoded byte at `d - 1`.
    fn raw_end_of(&self, d: usize) -> Option<usize> {
        match self {
            RawMap::Identity { r_start } => Some(*r_start as usize + d),
            RawMap::Segments(segs) => {
                let Some(prev) = d.checked_sub(1) else {
                    return segs.first().map(|s| s.r_start as usize);
                };
                match Self::seg_containing(segs, prev) {
                    Some(s) if s.is_one_to_one() => {
                        Some((s.r_start as usize) + (d - s.d_start as usize))
                    }
                    Some(s) if d == (s.d_start + s.d_len) as usize => {
                        Some((s.r_start + s.r_len) as usize)
                    }
                    _ => None,
                }
            }
        }
    }
}

/// Which smart-punctuation rewrites are on, so `build_raw_map` can undo them.
#[derive(Clone, Copy)]
pub(crate) struct Smart {
    pub quotes: bool,
    pub dashes: bool,
    pub ellipses: bool,
}

/// How many em and en dashes smart punctuation renders a run of `count`
/// hyphens as.
fn smart_dash_counts(count: usize) -> (usize, usize) {
    debug_assert!(count >= 2, "a lone hyphen is not a dash run");
    match count % 6 {
        0 | 3 => (count / 3, 0),
        2 | 4 => (0, count / 2),
        1 => (count / 3 - 1, 2),
        _ => (count / 3, 1),
    }
}

/// The em/en dash mix smart punctuation renders a run of `count` hyphens as.
pub(crate) fn smart_dash_run(count: usize) -> String {
    let (ems, ens) = smart_dash_counts(count);
    let mut buf = String::with_capacity(EM_DASH.len() * (ems + ens));
    for _ in 0..ems {
        buf.push_str(EM_DASH);
    }
    for _ in 0..ens {
        buf.push_str(EN_DASH);
    }
    buf
}

const EM_DASH: &str = "\u{2014}";
const EN_DASH: &str = "\u{2013}";

/// The raw and decoded lengths of the smart-punctuation rewrite at `raw[r]`.
fn smart_seg(raw: &[u8], r: usize, dec: &[u8], d: usize, smart: Smart) -> Option<(usize, usize)> {
    const ELLIPSIS: &str = "\u{2026}";
    match raw[r] {
        b'.' if smart.ellipses
            && raw[r..].starts_with(b"...")
            && dec[d..].starts_with(ELLIPSIS.as_bytes()) =>
        {
            Some((3, ELLIPSIS.len()))
        }
        b'-' if smart.dashes => {
            let count = 1 + crate::scanners::scan_ch_repeat(&raw[(r + 1)..], b'-');
            if count < 2 {
                return None;
            }
            let (ems, ens) = smart_dash_counts(count);
            let mut rest = &dec[d..];
            for (n, dash) in [(ems, EM_DASH), (ens, EN_DASH)] {
                for _ in 0..n {
                    rest = rest.strip_prefix(dash.as_bytes())?;
                }
            }
            Some((count, EM_DASH.len() * (ems + ens)))
        }
        c @ (b'"' | b'\'') if smart.quotes => {
            let curly: [&str; 2] = if c == b'"' {
                ["\u{201c}", "\u{201d}"]
            } else {
                ["\u{2018}", "\u{2019}"]
            };
            curly
                .iter()
                .find(|q| dec[d..].starts_with(q.as_bytes()))
                .map(|q| (1, q.len()))
        }
        _ => None,
    }
}

/// Align a Text node's decoded value with the raw source it came from.
///
/// At each raw offset the walk asks which transform applies, never whether the
/// bytes differ: raw `&amp;` starts with the byte it decodes to, so a
/// mismatch-directed walk would consume it and then choke on `amp;`.
///
/// Returns `None` when the value can't be reconstructed exactly; callers then
/// report no position rather than a guessed one that mis-slices the source.
fn build_raw_map(
    source: &[u8],
    r_start: usize,
    r_end: usize,
    decoded: &str,
    smart: Smart,
) -> Option<RawMap> {
    if r_start > r_end || r_end > source.len() {
        return None;
    }
    let raw = &source[r_start..r_end];
    let dec = decoded.as_bytes();
    if raw == dec {
        return Some(RawMap::Identity {
            r_start: r_start as u32,
        });
    }

    let mut segs: Vec<Seg> = Vec::new();
    let push_atomic = |segs: &mut Vec<Seg>, r: usize, r_len: usize, d: usize, d_len: usize| {
        segs.push(Seg {
            d_start: d as u32,
            d_len: d_len as u32,
            r_start: (r_start + r) as u32,
            r_len: r_len as u32,
        });
    };
    let extend_one_to_one = |segs: &mut Vec<Seg>, r: usize, d: usize| {
        if let Some(last) = segs.last_mut() {
            if last.is_one_to_one()
                && (last.d_start + last.d_len) as usize == d
                && (last.r_start + last.r_len) as usize == r_start + r
            {
                last.d_len += 1;
                last.r_len += 1;
                return;
            }
        }
        segs.push(Seg {
            d_start: d as u32,
            d_len: 1,
            r_start: (r_start + r) as u32,
            r_len: 1,
        });
    };

    // A continuation line's block prefix (`> `, indentation) produces no
    // decoded bytes. Content can't start with one: leading whitespace is
    // stripped and a leading `>` would have opened a blockquote.
    let skip_block_prefix = |segs: &mut Vec<Seg>, r: &mut usize, d: usize| {
        let prefix_start = *r;
        while *r < raw.len()
            && matches!(raw[*r], b' ' | b'\t' | b'>')
            && dec.get(d) != Some(&raw[*r])
        {
            *r += 1;
        }
        if *r > prefix_start {
            segs.push(Seg {
                d_start: d as u32,
                d_len: 0,
                r_start: (r_start + prefix_start) as u32,
                r_len: (*r - prefix_start) as u32,
            });
        }
    };

    // One predictable test keeps the smart arms off the hot path entirely for
    // the documents that don't enable them.
    let smart_any = smart.quotes || smart.dashes || smart.ellipses;
    let mut r = 0usize;
    let mut d = 0usize;
    while r < raw.len() {
        match raw[r] {
            b'&' => {
                let (len, value) = crate::scanners::scan_entity(&raw[r..]);
                if let Some(value) = value {
                    if dec[d..].starts_with(value.as_bytes()) {
                        push_atomic(&mut segs, r, len, d, value.len());
                        r += len;
                        d += value.len();
                        continue;
                    }
                }
            }
            b'\\' => {
                if let Some(&next) = raw.get(r + 1) {
                    if next.is_ascii_punctuation() && dec.get(d) == Some(&next) {
                        push_atomic(&mut segs, r, 2, d, 1);
                        r += 2;
                        d += 1;
                        continue;
                    }
                }
            }
            b'\r' if raw.get(r + 1) == Some(&b'\n') && dec.get(d) == Some(&b'\n') => {
                push_atomic(&mut segs, r, 2, d, 1);
                r += 2;
                d += 1;
                skip_block_prefix(&mut segs, &mut r, d);
                continue;
            }
            b'\n' | b'\r' if dec.get(d) == Some(&raw[r]) => {
                extend_one_to_one(&mut segs, r, d);
                r += 1;
                d += 1;
                skip_block_prefix(&mut segs, &mut r, d);
                continue;
            }
            b' ' | b'\t' if dec.get(d) != Some(&raw[r]) => {
                // Whitespace at the end of a line is dropped from the value.
                let run_start = r;
                while r < raw.len() && matches!(raw[r], b' ' | b'\t') {
                    r += 1;
                }
                if !matches!(raw.get(r), Some(b'\n') | Some(b'\r')) {
                    return None;
                }
                segs.push(Seg {
                    d_start: d as u32,
                    d_len: 0,
                    r_start: (r_start + run_start) as u32,
                    r_len: (r - run_start) as u32,
                });
                continue;
            }
            b'.' | b'-' | b'"' | b'\'' if smart_any => {
                if let Some((r_len, d_len)) = smart_seg(raw, r, dec, d, smart) {
                    push_atomic(&mut segs, r, r_len, d, d_len);
                    r += r_len;
                    d += d_len;
                    continue;
                }
            }
            0 => {
                // CommonMark replaces NUL with the replacement character.
                const REPLACEMENT: &str = "\u{FFFD}";
                if dec[d..].starts_with(REPLACEMENT.as_bytes()) {
                    push_atomic(&mut segs, r, 1, d, REPLACEMENT.len());
                    r += 1;
                    d += REPLACEMENT.len();
                    continue;
                }
            }
            _ => {}
        }
        if dec.get(d) != Some(&raw[r]) {
            return None;
        }
        extend_one_to_one(&mut segs, r, d);
        r += 1;
        d += 1;
    }
    if d != dec.len() {
        return None;
    }
    Some(RawMap::Segments(segs))
}

/// Source span and line/column pair for the decoded range `d_lo..d_hi`.
///
/// Line and column aren't optional: the exposed `position.*.offset` is derived
/// from them downstream, so a zero line would serialize garbage.
fn pos_for(
    map: &RawMap,
    cursor: &mut satteri_arena::LineIndexCursor<'_, '_>,
    d_lo: usize,
    d_hi: usize,
) -> Option<(u32, u32, u32, u32, u32, u32)> {
    let so = map.raw_start_of(d_lo)? as u32;
    let eo = map.raw_end_of(d_hi)? as u32;
    if eo < so {
        return None;
    }
    let (sl, sc) = cursor.offset_to_line_col(so);
    let (el, ec) = cursor.offset_to_line_col(eo);
    Some((so, eo, sl, sc, el, ec))
}

/// Fallback scan over a Text node's bytes: each match becomes a Link, and
/// everything left over — including characters stripped off a match's tail —
/// becomes a sibling Text node.
fn split_text_with_autolinks_fnr(
    arena: &mut Arena<Mdast>,
    text_id: u32,
    source_bytes: &[u8],
    cursor: Option<&mut satteri_arena::LineIndexCursor<'_, '_>>,
    smart: Smart,
) {
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let borrowed_text = arena.get_str(sr);
    let bytes = borrowed_text.as_bytes();

    let mut matches: Vec<(usize, usize, usize, String)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // Triggers are case-insensitive (`HTTP://`, `WWW.`).
        let b = bytes[i];
        let hit = match b {
            b'h' | b'H' | b'w' | b'W' => fnr_find_url(bytes, i),
            b'@' => fnr_find_email(bytes, i),
            _ => None,
        };
        if let Some((s, url_end, url, raw_end)) = hit {
            let last_end = matches.last().map_or(0, |m| m.2);
            if s >= last_end {
                matches.push((s, url_end, raw_end, url));
                i = raw_end;
                continue;
            }
        }
        i += 1;
    }

    if matches.is_empty() {
        return;
    }
    // Copy only once a rewrite is certain; the arena is mutated below.
    let text = borrowed_text.to_string();
    let bytes = text.as_bytes();

    // Built only after the no-match return: text full of `w`/`h`/`@` that
    // autolinks nothing shouldn't pay for the alignment.
    let node = arena.get_node(text_id);
    let (r_start, r_end) = (node.start_offset as usize, node.end_offset as usize);
    let map = cursor
        .as_ref()
        .and_then(|_| build_raw_map(source_bytes, r_start, r_end, &text, smart));
    if cursor.is_some() && map.is_none() {
        debug_assert!(
            false,
            "build_raw_map failed to reconstruct a text value from its source span"
        );
    }
    let mut cursor = cursor;
    let mut pos_for = |lo: usize, hi: usize| -> Option<(u32, u32, u32, u32, u32, u32)> {
        let map = map.as_ref()?;
        pos_for(map, cursor.as_deref_mut()?, lo, hi)
    };

    let mut new_children: Vec<u32> = Vec::new();
    let mut cursor = 0usize;
    for (s, url_end, raw_end, url) in matches {
        if s > cursor {
            let chunk = &text[cursor..s];
            let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
            let chunk_sr = arena.alloc_string(chunk);
            arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
            if let Some((so, eo, sl, sc, el, ec)) = pos_for(cursor, s) {
                arena.set_position(new_text_id, so, eo, sl, sc, el, ec);
            }
            new_children.push(new_text_id);
        }
        let link_id = arena.alloc_node(MdastNodeType::Link as u8);
        let url_sr = arena.alloc_string(&url);
        let link_data = LinkData {
            url: url_sr,
            title: StringRef::empty(),
        };
        arena.set_type_data(link_id, &link_data.to_bytes());
        let link_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let disp_sr = arena.alloc_string(&text[s..url_end]);
        arena.set_type_data(link_text_id, &disp_sr.as_bytes());
        if let Some((so, eo, sl, sc, el, ec)) = pos_for(s, url_end) {
            arena.set_position(link_id, so, eo, sl, sc, el, ec);
            arena.set_position(link_text_id, so, eo, sl, sc, el, ec);
        }
        arena.set_children(link_id, &[link_text_id]);
        new_children.push(link_id);
        // `findUrl` emits the `splitUrl` trail as its *own* text node, kept
        // separate from the text that follows (matching `findAndReplace`):
        // `.www.x. y` → `text("."), link, text("."), text(" y")`. (The
        // construct path instead leaves its trail merged with the following
        // text, but that's emitted in the firstpass, not here.)
        if raw_end > url_end {
            let trail_chunk = &text[url_end..raw_end];
            let trail_id = arena.alloc_node(MdastNodeType::Text as u8);
            let trail_sr = arena.alloc_string(trail_chunk);
            arena.set_type_data(trail_id, &trail_sr.as_bytes());
            if let Some((so, eo, sl, sc, el, ec)) = pos_for(url_end, raw_end) {
                arena.set_position(trail_id, so, eo, sl, sc, el, ec);
            }
            new_children.push(trail_id);
        }
        cursor = raw_end;
    }
    if cursor < bytes.len() {
        let chunk = &text[cursor..];
        let new_text_id = arena.alloc_node(MdastNodeType::Text as u8);
        let chunk_sr = arena.alloc_string(chunk);
        arena.set_type_data(new_text_id, &chunk_sr.as_bytes());
        if let Some((so, eo, sl, sc, el, ec)) = pos_for(cursor, bytes.len()) {
            arena.set_position(new_text_id, so, eo, sl, sc, el, ec);
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

#[cfg(feature = "mdx")]
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

#[cfg(test)]
mod tests {
    use super::{build_raw_map, RawMap, Smart};

    const OFF: Smart = Smart {
        quotes: false,
        dashes: false,
        ellipses: false,
    };
    const ON: Smart = Smart {
        quotes: true,
        dashes: true,
        ellipses: true,
    };

    fn map(source: &str, decoded: &str) -> RawMap {
        build_raw_map(source.as_bytes(), 0, source.len(), decoded, OFF).expect("map should build")
    }

    fn smart_map(source: &str, decoded: &str) -> RawMap {
        build_raw_map(source.as_bytes(), 0, source.len(), decoded, ON).expect("map should build")
    }

    #[test]
    fn raw_map_identity_is_allocation_free() {
        assert!(matches!(
            map("www.x.y", "www.x.y"),
            RawMap::Identity { r_start: 0 }
        ));
    }

    #[test]
    fn raw_map_spans_a_character_reference_whole() {
        // The reference occupies raw 1..6.
        let m = map("a&amp;b", "a&b");
        assert_eq!(m.raw_start_of(0), Some(0));
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(2), Some(6));
        assert_eq!(m.raw_end_of(3), Some(7));
    }

    #[test]
    fn raw_map_atomic_interior_has_no_position() {
        // `&fjlig;` decodes to two characters; the boundary between them
        // names no raw offset.
        let m = map("&fjlig;", "fj");
        assert_eq!(m.raw_start_of(0), Some(0));
        assert_eq!(m.raw_end_of(2), Some(7));
        assert_eq!(m.raw_start_of(1), None);
        assert_eq!(m.raw_end_of(1), None);
    }

    #[test]
    fn raw_map_spans_a_smart_dash_run_whole() {
        // `--` occupies raw 1..3 and decodes to one 3-byte en dash.
        let m = smart_map("a--b", "a\u{2013}b");
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(4), Some(3));
        // Interior of the atomic run names no raw offset.
        assert_eq!(m.raw_start_of(2), None);
        assert_eq!(m.raw_end_of(2), None);
    }

    #[test]
    fn raw_map_spans_a_long_dash_run_by_the_shared_formula() {
        // Five hyphens render as em + en, so the run is 5 raw to 6 decoded.
        let m = smart_map("a-----b", "a\u{2014}\u{2013}b");
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(7), Some(6));

        // Seven is the branch that subtracts: em + two en, 7 raw to 9 decoded.
        let m = smart_map("a-------b", "a\u{2014}\u{2013}\u{2013}b");
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(10), Some(8));
    }

    #[test]
    fn raw_map_spans_an_ellipsis_and_a_quote_whole() {
        let m = smart_map("a...b", "a\u{2026}b");
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(4), Some(4));

        let q = smart_map("a\"b\"", "a\u{201c}b\u{201d}");
        assert_eq!(q.raw_start_of(1), Some(1));
        assert_eq!(q.raw_end_of(4), Some(2));
    }

    #[test]
    fn raw_map_leaves_smart_bytes_alone_when_the_option_is_off() {
        // The same raw text decodes to itself, so it stays the identity map.
        assert!(matches!(map("a--b", "a--b"), RawMap::Identity { .. }));
        // And a curled value cannot be reconstructed without the option.
        assert!(build_raw_map("a--b".as_bytes(), 0, 4, "a\u{2013}b", OFF).is_none());
    }

    #[test]
    fn raw_map_prefers_the_escape_over_a_smart_dash() {
        // `\-` is an escape, so the run after it is only two hyphens.
        let m = smart_map("\\---", "-\u{2013}");
        assert_eq!(m.raw_end_of(1), Some(2));
        assert_eq!(m.raw_end_of(4), Some(4));
    }

    #[test]
    fn raw_map_excludes_a_continuation_prefix() {
        // The `> ` produces nothing, so it belongs to neither side.
        let m = map("a\n> b", "a\nb");
        assert_eq!(m.raw_end_of(2), Some(2));
        assert_eq!(m.raw_start_of(2), Some(4));
    }

    #[test]
    fn raw_map_handles_escapes_and_line_endings() {
        let m = map("a\\_b\r\nc", "a_b\nc");
        assert_eq!(m.raw_start_of(1), Some(1));
        assert_eq!(m.raw_end_of(2), Some(3));
        assert_eq!(m.raw_start_of(4), Some(6));
    }

    #[test]
    fn raw_map_refuses_to_guess() {
        assert!(build_raw_map(b"a&amp;b", 0, 7, "a&z", OFF).is_none());
        assert!(build_raw_map(b"abc", 0, 3, "abcd", OFF).is_none());
    }
}
