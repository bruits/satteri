# Fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [MDAST] (chaos)

**Input:** `"\t{uz-4\n\t"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "code",
      "lang": null,
      "meta": null,
      "value": "{uz-4",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 2,
          "column": 2,
          "offset": 8
        }
      }
    }
  ],
  "position": {
    "start": {
      "line": 1,
      "column": 1,
      "offset": 0
    },
    "end": {
      "line": 2,
      "column": 2,
   
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
      "offset": 8,
      "line": 2,
      "column": 2
    }
  },
  "children": [
    {
      "type": "code",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 6,
          "line": 1,
          "column": 7
        }
      },
      "lang": null,
      "meta": null,
      "v
```
## 2. [HAST] (chaos)

**Input:** `"\t880f\n\t"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "pre",
      "properties": {},
      "children": [
        {
          "type": "element",
          "tagName": "code",
          "properties": {},
          "children": [
            {
              "type": "text",
              "value": "880f\n"
            }
          ],
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
  
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
      "offset": 7,
      "line": 2,
      "column": 2
    }
  },
  "children": [
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 5,
          "line": 1,
          "column": 6
        }
      },
      "tagName": "pre",
      "properties": {
```
