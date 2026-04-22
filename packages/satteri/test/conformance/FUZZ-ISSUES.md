# Fuzz-discovered conformance issues

Found 10 unique issue(s) across 12 total failure(s).

## 1. [MDAST] (structured)

**Input:** `"ti4e8xg3kgf\n\n~~tv08e~~\n\n1. j0\n2. g46uctfta3z \n3. tkil38s33v\n4. l3iaei\n\n`mnspicvxi`\n\n| uypahiiq | iich | mwzpe | pj |\n| --- | --- | --- | --- |\n| efcfoy | cvghvqxhu | njreqoiocvpq | wxfluvuq |\n| ae | qmhpmprcjqsj | bbrtknbumd | ufztswmbdkf |\n\n**1n**\n\nn5pau2ksj74\n\n* *\n\n \n\n```html\nd301ow\n8vhs\n```\n\n- [x] 11y\n- [x] 7wyo0\n- [x] h44axxd61e9r"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "paragraph",
      "children": [
        {
          "type": "text",
          "value": "ti4e8xg3kgf",
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              "line": 1,
              "column": 12,
              "offset": 11
            }
          }
        }
      ],
      "position": {
        "start": {
          "line"
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
      "offset": 336,
      "line": 32,
      "column": 19
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
          "offset": 11,
          "line": 1,
          "column": 12
        }
      },
      "children": [
        {
       
```
## 2. [MDAST] (chaos)

**Input:** `"+\n "`

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
## 3. [MDAST] (chaos)

**Input:** `"\tl\n\t"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "code",
      "lang": null,
      "meta": null,
      "value": "l",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 2,
          "column": 2,
          "offset": 4
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
      "offset": 4,
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
          "offset": 2,
          "line": 1,
          "column": 3
        }
      },
      "lang": null,
      "meta": null,
      "v
```
## 4. [MDAST] (chaos)

**Input:** `"*  \n "`

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
              "column": 4,
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
      "offset": 5,
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
          "offset": 5,
          "line": 2,
          "column": 2
        }
      },
      "ordered": false,
      "start": null,
   
```
## 5. [MDAST] (chaos)

**Input:** `"h )qka\n|-"`

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
                  "value": "h )qka",
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
      "offset": 9,
      "line": 2,
      "column": 3
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
          "offset": 9,
          "line": 2,
          "column": 3
        }
      },
      "children": [
        {
          "ty
```
## 6. [MDAST] (chaos)

**Input:** `"\t<@9\\s\n\t"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "code",
      "lang": null,
      "meta": null,
      "value": "<@9\\s",
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
## 7. [HAST] (chaos)

**Input:** `"rv9\n-|"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "table",
      "properties": {},
      "children": [
        {
          "type": "text",
          "value": "\n"
        },
        {
          "type": "element",
          "tagName": "thead",
          "properties": {},
          "children": [
            {
              "type": "text",
              "value": "\n"
            },
            {
              "type": "element",
              "tagName": "tr",
      
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
      "offset": 6,
      "line": 2,
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
          "offset": 6,
          "line": 2,
          "column": 3
        }
      },
      "tagName": "p",
      "properties": {},
```
## 8. [HAST] (chaos)

**Input:** `"+\n "`

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
## 9. [HTML] (chaos)

**Input:** `">g\n\t>}0"`

**Expected (reference):**
```json
"<blockquote>\n<p>g\n>}0</p>\n</blockquote>"
```

**Actual (Sätteri):**
```json
"<blockquote>\n<p>g\n}0</p>\n</blockquote>"
```
## 10. [HTML] (chaos)

**Input:** `"<*\n-| \n5*"`

**Expected (reference):**
```json
"<table>\n<thead>\n<tr>\n<th>&lt;*</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>5*</td>\n</tr>\n</tbody>\n</table>"
```

**Actual (Sätteri):**
```json
"<p>&lt;*\n-|\n5*</p>"
```
