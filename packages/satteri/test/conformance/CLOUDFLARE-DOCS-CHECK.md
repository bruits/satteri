# Cloudflare docs conformance check

- Root: `/home/erika/Projects/cloudflare-docs`
- Features: MDX + frontmatter + directive
- Files: 8512
- MDAST: 8503 ok / 9 fail
- HAST:  8504 ok / 8 fail
- Parse errors: 0


## MDAST mismatches — 3 unique pattern(s)

### 7× `$.children[N].children[N].children[N].children`
- src/content/changelog/waf/2025-08-11-waf-release.mdx
  - $.children[8].children[1].children[9].children: array length 3 vs 7
- src/content/changelog/waf/2025-12-18-waf-release.mdx
  - $.children[5].children[1].children[1].children: array length 3 vs 7
- src/content/changelog/waf/2026-01-12-waf-release.mdx
  - $.children[5].children[1].children[1].children: array length 3 vs 7
  … and 4 more

### 1× `$.children[N].children[N].children[N].children[N].type`
- src/content/docs/analytics/graphql-api/getting-started/authentication/index.mdx
  - $.children[4].children[1].children[1].children[1].type: "mdxJsxFlowElement" vs "paragraph"

### 1× `$.children[N].children[N].children[N].children[N].children[N].children[N].children[N].children[N].attributes[N].value.va`
- src/content/partials/cloudflare-one/warp/add-split-tunnels-route.mdx
  - $.children[2].children[0].children[0].children[4].children[1].children[0].children[3].children[1].attributes[1].value.value: "{\n  \t\tbase: \"172.16.0.0/12\",\n  \t\tsubtract: [\"172.31.0.0/16\"]\n  \t}" vs "{\n  \t\t\tbase: \"172.16.0.0/12\",\n  \t\t\tsubtract: [\"172.31.0.0/16\"]\n  \


## HAST mismatches — 2 unique pattern(s)

### 7× `$.children[N].children[N].children[N].children`
- src/content/changelog/waf/2025-08-11-waf-release.mdx
  - $.children[14].children[1].children[9].children: array length 3 vs 7
- src/content/changelog/waf/2025-12-18-waf-release.mdx
  - $.children[8].children[1].children[1].children: array length 3 vs 7
- src/content/changelog/waf/2026-01-12-waf-release.mdx
  - $.children[8].children[1].children[1].children: array length 3 vs 7
  … and 4 more

### 1× `$.children[N].children[N].children[N].children[N].type`
- src/content/docs/analytics/graphql-api/getting-started/authentication/index.mdx
  - $.children[6].children[1].children[1].children[1].type: "mdxJsxFlowElement" vs "element"

