# Docs conformance check

- Root: `/home/erika/Projects/docs`
- Features: MDX + frontmatter + directive
- Files: 2417
- MDAST: 2415 ok / 2 fail
- HAST:  2417 ok / 0 fail
- Parse errors: 0


## MDAST mismatches — 2 unique pattern(s)

### 1× `$.children[N].children[N].children`
- src/content/docs/de/guides/cms/index.mdx
  - $.children[3].children[0].children: array length 4 vs 3

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].value`
- src/content/docs/ru/guides/testing.mdx
  - $.children[44].children[0].children[1].children[2].children[1].children[2].value: ": \"http://localhost" vs ": \""


