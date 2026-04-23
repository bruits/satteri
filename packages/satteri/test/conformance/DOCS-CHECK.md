# Docs conformance check

- Root: `/home/erika/Projects/docs`
- Features: MDX + frontmatter + directive
- Files: 2417
- MDAST: 2387 ok / 30 fail
- HAST:  2399 ok / 18 fail
- Parse errors: 0


## MDAST mismatches — 7 unique pattern(s)

### 10× `$.children[N].children[N].children[N].spread`
- src/content/docs/de/tutorial/4-layouts/2.mdx
  - $.children[10].children[2].children[1].spread: true vs false
- src/content/docs/en/tutorial/4-layouts/2.mdx
  - $.children[10].children[2].children[1].spread: true vs false
- src/content/docs/es/tutorial/4-layouts/2.mdx
  - $.children[10].children[2].children[1].spread: true vs false
  … and 7 more

### 9× `$.children[N].children[N].children[N].children[N].children`
- src/content/docs/en/tutorial/6-islands/4.mdx
  - $.children[18].children[0].children[2].children[2].children: array length 3 vs 1
- src/content/docs/fr/tutorial/6-islands/4.mdx
  - $.children[18].children[0].children[2].children[2].children: array length 3 vs 1
- src/content/docs/it/tutorial/6-islands/4.mdx
  - $.children[18].children[0].children[2].children[2].children: array length 3 vs 1
  … and 6 more

### 5× `$.children`
- src/content/docs/ar/basics/layouts.mdx
  - $.children: array length 46 vs 44
- src/content/docs/ko/guides/cms/builderio.mdx
  - $.children: array length 73 vs 27
- src/content/docs/pl/basics/layouts.mdx
  - $.children: array length 42 vs 41
  … and 2 more

### 3× `$.children[N].value`
- src/content/docs/es/guides/sessions.mdx
  - $.children[8].value: "/* TODO: add link to \n- Node: /es/guides/integrations-guide/node/#sesiones, \n vs "/* TODO: add link to \n - Node: /es/guides/integrations-guide/node/#sesiones, \
- src/content/docs/ja/recipes/build-forms-api.mdx
  - $.children[8].value: "/* ## 拡張: Zodでフォームを検証します\n\n[Zod form data](https://www.npmjs.com/package/zod-f vs "/* ## 拡張: Zodでフォームを検証します\n\n[Zod form data](https://www.npmjs.com/package/zod-f
- src/content/docs/ru/basics/layouts.mdx
  - $.children[40].value: "---\n// src/layouts/BlogPostLayout.astro\nimport BaseLayout from './BaseLayout. vs "---\n// src/layouts/BlogPostLayout.astro\nimport BaseLayout from './BaseLayout.

### 1× `$.children[N].children[N].children`
- src/content/docs/de/guides/cms/index.mdx
  - $.children[3].children[0].children: array length 4 vs 3

### 1× `$.children[N].children[N].children[N].value`
- src/content/docs/fr/reference/programmatic-reference.mdx
  - $.children[33].children[0].children[4].value: "\r\n" vs "\n"

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].value`
- src/content/docs/ru/guides/testing.mdx
  - $.children[44].children[0].children[1].children[2].children[1].children[2].value: ": \"http://localhost" vs ": \""


## HAST mismatches — 4 unique pattern(s)

### 9× `$.children[N].children[N].children[N].children[N].children`
- src/content/docs/en/tutorial/6-islands/4.mdx
  - $.children[34].children[0].children[5].children[5].children: array length 7 vs 3
- src/content/docs/fr/tutorial/6-islands/4.mdx
  - $.children[34].children[0].children[5].children[5].children: array length 7 vs 3
- src/content/docs/it/tutorial/6-islands/4.mdx
  - $.children[34].children[0].children[5].children[5].children: array length 7 vs 3
  … and 6 more

### 5× `$.children`
- src/content/docs/ar/basics/layouts.mdx
  - $.children: array length 87 vs 83
- src/content/docs/ko/guides/cms/builderio.mdx
  - $.children: array length 127 vs 45
- src/content/docs/pl/basics/layouts.mdx
  - $.children: array length 79 vs 77
  … and 2 more

### 2× `$.children[N].value`
- src/content/docs/es/guides/sessions.mdx
  - $.children[14].value: "/* TODO: add link to \n- Node: /es/guides/integrations-guide/node/#sesiones, \n vs "/* TODO: add link to \n - Node: /es/guides/integrations-guide/node/#sesiones, \
- src/content/docs/ja/recipes/build-forms-api.mdx
  - $.children[14].value: "/* ## 拡張: Zodでフォームを検証します\n\n[Zod form data](https://www.npmjs.com/package/zod-f vs "/* ## 拡張: Zodでフォームを検証します\n\n[Zod form data](https://www.npmjs.com/package/zod-f

### 2× `$.children[N].children[N].children[N].value`
- src/content/docs/fr/reference/programmatic-reference.mdx
  - $.children[64].children[0].children[4].value: "\r\n" vs "\n"
- src/content/docs/ru/basics/layouts.mdx
  - $.children[76].children[0].children[0].value: "---\n// src/layouts/BlogPostLayout.astro\nimport BaseLayout from './BaseLayout. vs "---\n// src/layouts/BlogPostLayout.astro\nimport BaseLayout from './BaseLayout.

