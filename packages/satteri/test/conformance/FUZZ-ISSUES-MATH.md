# Math fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [MATH-HAST] (structured)

**Input:** `"---\n\n---\n\n1v0\n\n##### obxkpho\n\na $668^$\n\n$$\nz{ q}g}\n$$\n\n> 5hyxhuc"`

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
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 1,
          "column": 4,
          "offset": 3
        }
      }
    },
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "tagName": "hr",
      "properties": {},
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
      "offset": 64,
      "line": 15,
      "column": 10
    }
  },
  "children": [
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 10,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 13,
          "line": 5,
          "column": 4
        }
      },
      "tagName": "p",
      "properties"
```
## 2. [MATH-HTML] (structured)

**Input:** `"---\n\n$$\n\\int\n$$\n\n#### tgvm2f\n\n---"`

**Expected (reference):**
```json
"<hr />\n<pre><code class=\"language-math math-display\">\\int</code></pre>\n<h4>tgvm2f</h4>\n<hr />"
```

**Actual (Sätteri):**
```json
""
```
