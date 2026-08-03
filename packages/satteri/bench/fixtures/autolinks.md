# GFM autolinks

A fixture for the autolink paths specifically. The shared `markdown.md` fixture
has no live autolink triggers at all — its `@` characters sit in fenced code and
every `https://` is inside a `](…)` destination or a reference definition — so
it cannot show a regression in either the first-pass scanner or the
find-and-replace post-pass.

## Bare triggers, no brackets

Visit www.commonmark.org for the spec, or https://github.com/bruits/satteri for
the source. Mail a@b.co or someone.else+tag@sub.example.org and they will
probably not answer. See http://example.com/a/b/c?q=1&r=2#frag and
HTTPS://EXAMPLE.COM/UPPER for the case-insensitive triggers.

Trailing punctuation is trimmed on the way out: www.example.com. And
www.example.com, and www.example.com! And (www.example.com) and
_www.example.com_ and ~www.example.com~ and www.example.com/a_(b)_c.

Paths run until whitespace: https://example.com/very/long/path/with-many-segments/and-a-query?a=1&b=2&c=3
and www.example.com/another/path/that/keeps/going/for/a/while/index.html.

## Triggers that never become links

This paragraph is dense in trigger bytes and produces nothing: what a wharf,
whatever the weather, who watches the watchmen, when we went where. The word
http on its own, and https, and www, and www. with nothing after it, and
http:// with no host at all. An @ on its own, and a @ b, and @start, and
trailing@, and h@w, and localhost@ and @example.

More of the same, because the scanner has to reject each one individually:
whether the whole thing works, wherever whichever whenever, however hither
thither, hash hush harsh, wash wish wush, http-ish and https-like and www-ish
and mailto-flavoured text with no colon.

Host rules reject these too: http://localhost and http://你好.cn and
www.nodot and a@b (no dot in the domain) and a@b.c- and a@b.c_ at the end.

## Brackets around triggers

An unclosed opener blocks the tokenizer, so these take the find-and-replace
path: [www.example.com and [http://example.com and [a@b.co and ![www.example.com
and [foo][www.example.com] and [www.example.com]( and [https://example.com](.

A closed-but-unresolved opener does not block it: [a] www.example.com and
![a] http://example.com and [a](/b) www.example.com and [a [b](/c) www.example.com.

Brackets that belong to another construct never count: [a `]` www.example.com
and `[` www.example.com and [a ``]`` www.example.com and ``[`` www.example.com
and <span a='['> www.example.com and [a <http://q.r/]> www.example.com.

## Triggers inside link destinations

Resolved destinations swallow the trigger: [one](https://example.com/a)x and
[two](www.example.com/b)y and [three](mailto:a@b.co)z and
[four](https://example.com/c "title")w.

Unresolved ones do not, so the trigger runs past the closing paren:
[[x]](https://example.com/a)x and [[x]](www.example.com/b)y and
[foo][bar](https://example.com/c)z and [[a](/b)](https://example.com/d)w.

[x]: /x
[bar]: /bar

## Entities, escapes and continuations

Character references decode before the post-pass sees them:
[www.example.com/&amp;a and [http://example.com/p&amp;q and
[&#104;ttp://example.com/e and [www.example.com/a\_b and [www.example.com/a\*b.

| a | b |
| - | - |
| [www.x.y\|z | [http://x.y\|z |
| plain www.x.y | plain http://x.y |

> [a
> www.example.com/in-a-blockquote
> and http://example.com/too

- [a
  www.example.com/in-a-list
- what a whopper of a list item with no link in it at all

## Mixed prose

Sätteri parses www.commonmark.org and https://spec.commonmark.org/0.31.2/ the
same way remark does, which matters because the two pipelines that produce
autolinks disagree about what a link is: the tokenizer sees raw source, the
find-and-replace transform sees decoded text. A document like this one, which
mixes [reference links][ref], inline [links](https://example.com), pointed
autolinks <https://example.com>, and bare www.example.com triggers, exercises
both.

[ref]: https://example.com/ref

Contact: docs@example.com, support@example.com, or file an issue at
https://github.com/bruits/satteri/issues. What we will not do is reply from
whatever address the whois record has, which is where most of the `w` bytes in
this paragraph come from.
