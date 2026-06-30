---
cargo/satteri-mdxjs: patch
---

Preserve significant whitespace between adjacent JSX elements (and inside
whitespace-only elements like `<em> </em>`) when JSX is parsed inside an
expression, such as an attribute expression. Previously a space-only text node
with no newline was dropped, so `<C d={<><a/> <b/></>} />` lost the `" "` and
rendered the two elements directly adjacent.
