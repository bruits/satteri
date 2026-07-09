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

/// Find-and-replace fallback for GFM autolink literals — the mdast-tree
/// transform equivalent of `mdast-util-gfm-autolink-literal`'s
/// `transformGfmAutolinkLiterals`. The inline construct in `firstpass.rs`
/// handles the common case (URL bytes consumed during tokenization with
/// source positions); this pass picks up URL/email patterns that survived
/// in plain Text nodes — typically because the construct path didn't fire
/// (e.g. preceded by a digit, inside a previously-failed `<...>` autolink,
/// across container prefixes). All Links emitted here are position-less,
/// matching `findAndReplace`'s behavior.
pub(crate) fn gfm_autolink_literal_pass(arena: &mut Arena<Mdast>, source_bytes: &[u8]) {
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
    for node_id in candidates {
        split_text_with_autolinks_fnr(arena, node_id, source_bytes);
    }
}

/// `previous()` in `mdast-util-gfm-autolink-literal`: prev char must be
/// whitespace, punctuation, or start-of-string. Stricter than the
/// construct's `previousProtocol` (`!alphabetic`), since digits and
/// non-ASCII letters fail.
fn fnr_prev_ok(bytes: &[u8], ix: usize) -> bool {
    if ix == 0 {
        return true;
    }
    let prev = bytes[ix - 1];
    if prev < 0x80 {
        return prev.is_ascii_whitespace() || prev.is_ascii_punctuation();
    }
    // Decode the last char to apply Unicode whitespace/punctuation rules
    // (matches the `\s` / `\p{P}` / `\p{S}` lookbehind in the regex).
    match core::str::from_utf8(&bytes[ix.saturating_sub(4)..ix]) {
        Ok(s) => {
            let c = s.chars().last().unwrap_or(' ');
            c.is_whitespace() || !c.is_alphanumeric()
        }
        Err(_) => true,
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
    if !fnr_prev_ok(bytes, s) {
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
    while s < ix && !fnr_prev_ok(bytes, s) {
        s += 1;
    }
    if s >= ix {
        return None;
    }
    let addr = core::str::from_utf8(&bytes[s..e]).ok()?;
    Some((s, e, format!("mailto:{addr}"), e))
}

/// FNR-style scan over a Text node's bytes. Emits position-less Links for
/// each match; left-over text becomes plain Text nodes between/around them.
/// `findUrl` returns `[link, text(trail)]` when splitUrl strips trailing
/// chars — `findAndReplace` then inserts those as adjacent siblings,
/// keeping the trail distinct from the surrounding text. Mirror that.
fn split_text_with_autolinks_fnr(arena: &mut Arena<Mdast>, text_id: u32, source_bytes: &[u8]) {
    let data = arena.get_type_data(text_id);
    if data.is_empty() {
        return;
    }
    let sr = StringRef::from_bytes(data);
    let text = arena.get_str(sr).to_string();
    let bytes = text.as_bytes();

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

    // Per `mdast-util-gfm-autolink-literal`'s `findAndReplace`, links
    // emitted here are intentionally position-less — even though they
    // span a known source range, the F&R transform doesn't carry source
    // offsets. We mirror that to match REF exactly on inputs where the
    // construct-level autolink tokenizer didn't fire (e.g. autolinks
    // preceded by `[`). Don't emit positions on the new nodes.
    let _ = source_bytes;
    let pos_for =
        |_chunk_lo: usize, _chunk_hi: usize| -> Option<(u32, u32, u32, u32, u32, u32)> { None };

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
