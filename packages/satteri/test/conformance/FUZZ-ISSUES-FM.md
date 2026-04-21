# Frontmatter fuzz-discovered conformance issues

Found 8 unique issue(s) across 174 total failure(s).

## 1. [FM-HAST] (structured)

**Input:** `"---\nexoj_tzy: 8794\n---\n\n**cu70l**\n\n**2hf3t9ml**\n\n> dwfx2ju"`

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
          "tagName": "strong",
          "properties": {},
          "children": [
            {
              "type": "text",
              "value": "cu70l",
              "position": {
                "start": {
                  "line": 5,
                  "column": 3,
                  "offset": 26
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
      "offset": 58,
      "line": 9,
      "column": 10
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 24,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 33,
          "line": 5,
          "column": 10
    
```
## 2. [FM-HAST] (structured)

**Input:** `"---\nlsaz: ofyl\n---\n\n> sj"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "blockquote",
      "properties": {},
      "children": [
        {
          "type": "text",
          "value": "\n"
        },
        {
          "type": "element",
          "tagName": "p",
          "properties": {},
          "children": [
            {
              "type": "text",
              "value": "sj",
              "position": {
                "start": {
                  "line": 5,
             
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
      "offset": 24,
      "line": 5,
      "column": 5
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 20,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 24,
          "line": 5,
          "column": 5
      
```
## 3. [FM-HAST] (structured)

**Input:** `"+++\nkfb = true\n+++\n\n## 76vc\n\nr"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "h2",
      "properties": {},
      "children": [
        {
          "type": "text",
          "value": "76vc",
          "position": {
            "start": {
              "line": 5,
              "column": 4,
              "offset": 23
            },
            "end": {
              "line": 5,
              "column": 8,
              "offset": 27
            }
          }
        }
      ],
      "position":
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
      "offset": 30,
      "line": 7,
      "column": 2
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 20,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 27,
          "line": 5,
          "column": 8
      
```
## 4. [FM-HAST] (structured)

**Input:** `"+++\nrlujpku = mvqj\n+++\n\n## mcepkfeh2\n\n`qh`\n\n***\n\n---\n\n1555lxfeb "`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "element",
      "tagName": "h2",
      "properties": {},
      "children": [
        {
          "type": "text",
          "value": "mcepkfeh2",
          "position": {
            "start": {
              "line": 5,
              "column": 4,
              "offset": 27
            },
            "end": {
              "line": 5,
              "column": 13,
              "offset": 36
            }
          }
        }
      ],
      "posi
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
      "line": 13,
      "column": 11
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 24,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 36,
          "line": 5,
          "column": 13
   
```
## 5. [FM-HAST] (structured)

**Input:** `"+++\nxep = \"8s1jjqykv\"\n+++\n\nbe80 vmiahj\n\n___\n\n![hqjlngkmww](https://example.com/muc)\n\n*igmf7*\n\n**ars**\n\n```python\nbqfjli0jnx\n```"`

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
          "value": "be80 vmiahj",
          "position": {
            "start": {
              "line": 5,
              "column": 1,
              "offset": 27
            },
            "end": {
              "line": 5,
              "column": 12,
              "offset": 38
            }
          }
        }
      ],
      "pos
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
      "offset": 127,
      "line": 17,
      "column": 4
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 27,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 38,
          "line": 5,
          "column": 12
   
```
## 6. [FM-HAST] (structured)

**Input:** `"+++\nw__umgw = true\n+++\n\n---\n\n# krghhsiwwa3\n\n- [ ] 5d\n- [x] 25mwpx2m jdz\n- [ ]  kmdlif\n\nkw2ao1q9\n\nojj30bdl9kc\n\n~~76peo~~\n\n283qq3\n\n***"`

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
          "offset": 24
        },
        "end": {
          "line": 5,
          "column": 4,
          "offset": 27
        }
      }
    },
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "tagName": "h1",
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
      "offset": 132,
      "line": 21,
      "column": 4
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 24,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 27,
          "line": 5,
          "column": 4
    
```
## 7. [FM-HAST] (structured)

**Input:** `"+++\ny_ = true\nwchfl = 2423\n+++\n\n* eq3j54q295*\n\n___\n\nbm3e\n\ncprj7"`

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
          "children": [
            {
              "type": "text",
              "value": "eq3j54q295*",
              "position": {
                "start": {
                  "line": 6,
           
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
      "offset": 63,
      "line": 12,
      "column": 6
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 32,
          "line": 6,
          "column": 1
        },
        "end": {
          "offset": 45,
          "line": 6,
          "column": 14
    
```
## 8. [FM-HAST] (structured)

**Input:** `"+++\nbvykr = false\n+++\n\n*pi*\n\n6rtstmt5\n\n> ld4\n\n*0teuglu2fuj*\n\n8bh7kpitxqk\n\n1. vqsxtme2ob\n\n[9bb76](https://example.com/w)"`

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
          "tagName": "em",
          "properties": {},
          "children": [
            {
              "type": "text",
              "value": "pi",
              "position": {
                "start": {
                  "line": 5,
                  "column": 2,
                  "offset": 24
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
      "offset": 119,
      "line": 17,
      "column": 31
    }
  },
  "children": [
    {
      "type": "text",
      "value": "\n"
    },
    {
      "type": "element",
      "position": {
        "start": {
          "offset": 23,
          "line": 5,
          "column": 1
        },
        "end": {
          "offset": 27,
          "line": 5,
          "column": 5
   
```
