# Fuzz-discovered conformance issues

Found 8 unique issue(s) across 18 total failure(s).

## 1. [MDAST] (structured)

**Input:** `"1. o4yy\n2. gearifow7ya\n3. k0pj p\n4. i3q3wye\n\n1. lo\n2. 8b8r\n3. 9ouopw9hozhx\n4. sgl63u\n5. 9af w3lgy\n\n1. ab"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "list",
      "ordered": true,
      "start": 1,
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
                  "value": "o4yy",
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
      "offset": 104,
      "line": 12,
      "column": 6
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
          "offset": 104,
          "line": 12,
          "column": 6
        }
      },
      "ordered": true,
      "start": 1,
 
```
## 2. [MDAST] (structured)

**Input:** `"- yhynnfxx5n8z\n\n- xr8f9m\n- zec1va7e3o\n- lkvz6y4kbwh\n- b\n- 3sfmx\n\n3n"`

**Expected (reference):**
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
                  "value": "yhynnfxx5n8z",
                  "position": {
                    "start"
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
      "offset": 67,
      "line": 9,
      "column": 3
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
          "offset": 63,
          "line": 7,
          "column": 8
        }
      },
      "ordered": false,
      "start": null,
 
```
## 3. [MDAST] (structured)

**Input:** `"[9a1lwk ](https://example.com/zg)\n\n9w5twsmi0\n\n58w5698i9og\n\n###### kr\n\n- aufy ja wf\n- omz \n- jkztga\n\n- d\n\n###### xfo6uyr \n\n> l80wv7ae\n\n~~x~~\n\n#### zmtzrsj"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "paragraph",
      "children": [
        {
          "type": "link",
          "title": null,
          "url": "https://example.com/zg",
          "children": [
            {
              "type": "text",
              "value": "9a1lwk ",
              "position": {
                "start": {
                  "line": 1,
                  "column": 2,
                  "offset": 1
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
      "offset": 153,
      "line": 21,
      "column": 13
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
          "offset": 33,
          "line": 1,
          "column": 34
        }
      },
      "children": [
        {
       
```
## 4. [MDAST] (structured)

**Input:** `"[ 68kore8moo](https://example.com/apjubs)\n\n- boetbr\n- 7rc\n- z e7p4zj\n- wvifyjfd\n\n- 1a9d6bb17\n\n`iggydmd`\n\n1. ofrzwqkj04\n2. v\n\n# 0s7mwb5zjyz\n\n- [ ] j"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "paragraph",
      "children": [
        {
          "type": "link",
          "title": null,
          "url": "https://example.com/apjubs",
          "children": [
            {
              "type": "text",
              "value": " 68kore8moo",
              "position": {
                "start": {
                  "line": 1,
                  "column": 2,
                  "offset": 1
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
      "offset": 147,
      "line": 17,
      "column": 8
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
          "offset": 41,
          "line": 1,
          "column": 42
        }
      },
      "children": [
        {
        
```
## 5. [MDAST] (structured)

**Input:** `"6sfsu58e3\n\n##### uh0w29uxb\n\n### egz7dt\n\n*ybqg8axfxh69*\n\n*s2em*\n\n1. r7xonjsl4p\n2. v7\n3. npu02hlcr\n\n1. 6\n2. 3zdnoc\n3. 6cy\n4. wry4\n\n`jyt`"`

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
          "value": "6sfsu58e3",
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              "line": 1,
              "column": 10,
              "offset": 9
            }
          }
        }
      ],
      "position": {
        "start": {
          "line": 1
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
      "offset": 134,
      "line": 20,
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
          "offset": 9,
          "line": 1,
          "column": 10
        }
      },
      "children": [
        {
         
```
## 6. [MDAST] (structured)

**Input:** `"wu81g vfj\n\nxvoj6\n\n```python\n59=y03gjij\n```\n\n- [ ] 1fhkzsx1m\n\n- vs\n\nbivuhqn\n\nxeok3"`

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
          "value": "wu81g vfj",
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              "line": 1,
              "column": 10,
              "offset": 9
            }
          }
        }
      ],
      "position": {
        "start": {
          "line": 1
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
      "offset": 81,
      "line": 15,
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
          "offset": 9,
          "line": 1,
          "column": 10
        }
      },
      "children": [
        {
          
```
## 7. [MDAST] (structured)

**Input:** `"###### o8 pefd\n\n- e9m9z99 hys\n- rs9\n- xv762tdxk9\n- 28\n\n- 24le5dluqs\n- nzx\n- ybb\n- qinwp5ybzk6\n\n> w8291\n\n- 8w\n- izgeq4u\n- gawj6"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "heading",
      "depth": 6,
      "children": [
        {
          "type": "text",
          "value": "o8 pefd",
          "position": {
            "start": {
              "line": 1,
              "column": 8,
              "offset": 7
            },
            "end": {
              "line": 1,
              "column": 15,
              "offset": 14
            }
          }
        }
      ],
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
      "offset": 126,
      "line": 17,
      "column": 8
    }
  },
  "children": [
    {
      "type": "heading",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 14,
          "line": 1,
          "column": 15
        }
      },
      "depth": 6,
      "children": [
  
```
## 8. [MDX-MDAST] (structured)

**Input:** `"*x16dyk3iub*\n\n- jq\n- 3wb3ve\n- lwhswx\n- iv18m e\n\n- da\n- vr0hj4t5\n\n`yji`\n\n{1 + 2}"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "paragraph",
      "children": [
        {
          "type": "emphasis",
          "children": [
            {
              "type": "text",
              "value": "x16dyk3iub"
            }
          ]
        }
      ]
    },
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
       
```

**Actual (Sätteri):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "paragraph",
      "children": [
        {
          "type": "emphasis",
          "children": [
            {
              "type": "text",
              "value": "x16dyk3iub"
            }
          ]
        }
      ]
    },
    {
      "type": "list",
      "ordered": false,
      "start": null,
      "spread": true,
      "children": [
        {
          "type": "listItem",
          "spread": true,
          "checked": null,
        
```
