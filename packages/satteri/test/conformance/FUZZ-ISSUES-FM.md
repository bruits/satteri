# Frontmatter fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [FM-HAST] (structured)

**Input:** `"---\nxnhouc: xmyfu\n---\n\n---\n\n**i**\n\n---"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "hr",
      "properties": {},
      "children": [],
      "position": {
        "start": {
          "line": 5,
          "column": 1,
          "offset": 23
        },
        "end": {
          "line": 5,
          "column": 4,
          "offset": 26
        }
      }
    },
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "tagName": "p",
      "properties": {},
      
```

**Actual (Sätteri):**
```json
{
  "type": "root",
  "position": {
    "start": {
      "offset": 0,
      "line": 1,
      "column": 1
    },
    "end": {
      "offset": 38,
      "line": 9,
      "column": 4
    }
  },
  "children": []
}
```
## 2. [FM-HAST] (structured)

**Input:** `"+++\nt = \"072ol\"\nm = false\nulbtmpx = false\ny = true\nzax = 291\n+++\n\n![glsx](https://example.com/qiulplqepzwk)\n\n*jzu83k4*\n\n---\n\n```html\n6 2\n```\n\n---"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "p",
      "properties": {},
      "children": [
        {
          "type": "element",
          "tagName": "img",
          "properties": {
            "src": "https://example.com/qiulplqepzwk",
            "alt": "glsx"
          },
          "children": [],
          "position": {
            "start": {
              "line": 9,
              "column": 1,
              "offset": 66
            },
            "
```

**Actual (Sätteri):**
```json
{
  "type": "root",
  "position": {
    "start": {
      "offset": 0,
      "line": 1,
      "column": 1
    },
    "end": {
      "offset": 145,
      "line": 19,
      "column": 4
    }
  },
  "children": [
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 66,
          "line": 9,
          "column": 1
        },
        "end": {
          "offset": 107,
          "line": 9,
          "column": 42
        }
      },
      "tagName": "p",
      "propertie
```
