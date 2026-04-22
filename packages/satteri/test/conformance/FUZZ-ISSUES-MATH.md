# Math fuzz-discovered conformance issues

Found 1 unique issue(s) across 1 total failure(s).

## 1. [MATH-MDAST] (structured)

**Input:** `"$$\nmv-c\n$$\n\n- mn\n- l1iourp7f88\n-  \n\n> w1dz3h5h1\n\n[pc cecu](https://example.com/aacdqjq)\n\n- c3ctzf639he\n- v41hnvkigub\n- 50zpreq9pxbl\n- 2fhqbplnpo7\n\n- gbchwud92hb\n- 19j\n- fwu9aeua\n- bw3myn\n- r6kd aiptw\n\n**28k34dvs**\n\n$$\nzn4ydh-w=\n$$"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "math",
      "meta": null,
      "value": "mv-c",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 3,
          "column": 3,
          "offset": 10
        }
      }
    },
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": false,
      "children": [
        {
          "type": "listItem",
          "s
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
      "offset": 230,
      "line": 28,
      "column": 3
    }
  },
  "children": [
    {
      "type": "math",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 10,
          "line": 3,
          "column": 3
        }
      },
      "meta": null,
      "value": "mv-c"
  
```
