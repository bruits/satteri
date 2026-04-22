# Frontmatter fuzz-discovered conformance issues

Found 1 unique issue(s) across 1 total failure(s).

## 1. [FM-MDAST] (structured)

**Input:** `"+++\nu = 2574\ndjzjkwn_clr = dkaon\nq = h\n+++\n\n##### b055e\n\n*q52*\n\n`eirwwirgxtm`\n\n- 2l\n- av9o5yeg\n\nhtomsw1j\n\n1. 2or jh41tsv\n2.  \n3. tnqnrxznve\n4. m4m21kh04bsn"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "toml",
      "value": "u = 2574\ndjzjkwn_clr = dkaon\nq = h",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 5,
          "column": 4,
          "offset": 42
        }
      }
    },
    {
      "type": "heading",
      "depth": 5,
      "children": [
        {
          "type": "text",
          "value": "b055e",
          "position": {
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
      "offset": 155,
      "line": 21,
      "column": 16
    }
  },
  "children": [
    {
      "type": "toml",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 42,
          "line": 5,
          "column": 4
        }
      },
      "value": "u = 2574\ndjzjkwn_clr = dka
```
