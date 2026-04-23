# Fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [MDAST] (chaos)

**Input:** `"~\n-|\n)*far"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "table",
      "align": [
        null
      ],
      "children": [
        {
          "type": "tableRow",
          "children": [
            {
              "type": "tableCell",
              "children": [
                {
                  "type": "text",
                  "value": "~",
                  "position": {
                    "start": {
                      "line": 1,
                      "column": 1,
                    
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
      "offset": 10,
      "line": 3,
      "column": 6
    }
  },
  "children": [
    {
      "type": "paragraph",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 10,
          "line": 3,
          "column": 6
        }
      },
      "children": [
        {
          "
```
## 2. [HTML] (chaos)

**Input:** `"~~~<c# "`

**Expected (reference):**
```json
"<pre><code class=\"language-<c#\"></code></pre>"
```

**Actual (Sätteri):**
```json
"<pre><code class=\"language-&lt;c#\"></code></pre>"
```
