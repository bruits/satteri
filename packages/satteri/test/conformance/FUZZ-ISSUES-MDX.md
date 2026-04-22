# MDX fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [MDX-EVAL] mismatch (structured)

**Input:** `"- g3ivbwilf\n- 8c3hiq60fs\n\n- f1\n- 6s3\n\n###### p47ybxa\n\n*yj*\n\n###### 488c\n\n<Box xqxyizn={53}/>\n\n##### ph2fdlpf"`

**@mdx-js/mdx:** `<ul><li><p>g3ivbwilf</p></li><li><p>8c3hiq60fs</p></li><li><p>f1</p></li><li><p>6s3</p></li></ul><h6>p47ybxa</h6><p><em>yj</em></p><h6>488c</h6><section></section><h5>ph2fdlpf</h5>`

**Sätteri:** `<ul><li>g3ivbwilf</li><li>8c3hiq60fs</li><li>f1</li><li>6s3</li></ul><h6>p47ybxa</h6><p><em>yj</em></p><h6>488c</h6><section></section><h5>ph2fdlpf</h5>`
## 2. [MDX-EVAL] mismatch (structured)

**Input:** `"**59twwd**\n\n- ri y8leb918\n\n- 8rptrc4lq8o\n\n`wfufdggs`\n\n{-338}\n\n{437}\n\n{/* comment */}"`

**@mdx-js/mdx:** `<p><strong>59twwd</strong></p><ul><li><p>ri y8leb918</p></li><li><p>8rptrc4lq8o</p></li></ul><p><code>wfufdggs</code></p>-338
437`

**Sätteri:** `<p><strong>59twwd</strong></p><ul><li>ri y8leb918</li><li>8rptrc4lq8o</li></ul><p><code>wfufdggs</code></p>-338
437`
