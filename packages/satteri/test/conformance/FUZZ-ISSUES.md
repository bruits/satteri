# Fuzz-discovered conformance issues

Found 5 unique issue(s) across 8 total failure(s).

## 1. [MDAST] (structured)

**Input:** `"- pv02y\n- hsfjvctjgw60\n\n  x5j77edfop\n\n`bdm`\n\n![qezmegvjb](https://example.com/rt)\n\n`mkjalex`"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": false,
      "children": [
        {
          "type": "listItem",
          "spread": false,
          "checked": null,
          "children": [
            {
              "type": "paragraph",
              "children": [
                {
                  "type": "text",
                  "value": "pv02y",
                  "position": {
                    "start": {
  
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
      "offset": 92,
      "line": 10,
      "column": 10
    }
  },
  "children": [
    {
      "type": "list",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 36,
          "line": 4,
          "column": 13
        }
      },
      "ordered": false,
      "start": null
```
## 2. [MDAST] (chaos)

**Input:** `"*\n "`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": false,
      "children": [
        {
          "type": "listItem",
          "spread": false,
          "checked": null,
          "children": [],
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              "line": 1,
              "column": 2,
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
      "offset": 3,
      "line": 2,
      "column": 2
    }
  },
  "children": [
    {
      "type": "list",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 3,
          "line": 2,
          "column": 2
        }
      },
      "ordered": false,
      "start": null,
   
```
## 3. [HAST] (chaos)

**Input:** `"h!-\n{<}\n|-"`

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
          "type": "text",
          "value": "h!-",
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
        }
      ],
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
      "offset": 10,
      "line": 3,
      "column": 3
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
          "offset": 10,
          "line": 3,
          "column": 3
        }
      },
      "tagName": "p",
      "properties": {
```
## 4. [HAST] (chaos)

**Input:** `"-\n\t"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "ul",
      "properties": {},
      "children": [
        {
          "type": "text",
          "value": "\n"
        },
        {
          "type": "element",
          "tagName": "li",
          "properties": {},
          "children": [],
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              
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
      "offset": 3,
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
          "offset": 3,
          "line": 2,
          "column": 2
        }
      },
      "tagName": "ul",
      "properties": {}
```
## 5. [MDX-MDAST] (structured)

**Input:** `"- id48cva\n- 9g0\n- bc9vu\n- qmsibdsu0\n\n  4pj 38yd 7aci2\n\n{1 + 2}\n\n{`7jsrvi9n`}"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": false,
      "children": [
        {
          "type": "listItem",
          "spread": false,
          "checked": null,
          "children": [
            {
              "type": "paragraph",
              "children": [
                {
                  "type": "text",
                  "value": "id48cva"
                }
              ]
            }
          ]
    
```

**Actual (Sätteri):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": true,
      "children": [
        {
          "type": "listItem",
          "spread": false,
          "checked": null,
          "children": [
            {
              "type": "paragraph",
              "children": [
                {
                  "type": "text",
                  "value": "id48cva"
                }
              ]
            }
          ]
     
```
