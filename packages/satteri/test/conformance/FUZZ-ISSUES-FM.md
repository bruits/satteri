# Frontmatter fuzz-discovered conformance issues

Found 5 unique issue(s) across 7 total failure(s).

## 1. [FM-MDAST] (structured)

**Input:** `"---\ncdqyvrbixj: 2722\new: \"d4obg\"\nh: y\ngvgvzg_g: \"uc\"\nzlp_: \"kd\"\n---\n\n> m3yr\n\n- scwdun1rnug8\n- uy68znkz\n- cmqh\n- 0elbh55w\n\n- [ ] yt4smobvtq\n\n![z](https://example.com/g)\n\n| xni | zxxdh |\n| --- | --- |\n| ifp | rmqvveg |\n| vlxog | v |"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "yaml",
      "value": "cdqyvrbixj: 2722\new: \"d4obg\"\nh: y\ngvgvzg_g: \"uc\"\nzlp_: \"kd\"",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 7,
          "column": 4,
          "offset": 67
        }
      }
    },
    {
      "type": "blockquote",
      "children": [
        {
          "type": "paragraph",
          "children": [
    
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
      "line": 23,
      "column": 14
    }
  },
  "children": [
    {
      "type": "yaml",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 67,
          "line": 7,
          "column": 4
        }
      },
      "value": "cdqyvrbixj: 2722\new: \"d4o
```
## 2. [FM-MDAST] (structured)

**Input:** `"+++\nszm = true\ngeqxw_cjpyba = 2552\njxybp = asjfjoxjkard\n+++\n\n#### n48\n\n*hg89k*\n\n```js\n6h lflys\n```\n\n**vuae**\n\n1. 8t 202nto69y\n2. 8t9uw9gy \n3. 82wj\n\n1. jg097uy\n2. yf1a 4u\n3. 5b\n4. tcimwt2euk1f\n\n*9916p5*"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "toml",
      "value": "szm = true\ngeqxw_cjpyba = 2552\njxybp = asjfjoxjkard",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 5,
          "column": 4,
          "offset": 59
        }
      }
    },
    {
      "type": "heading",
      "depth": 4,
      "children": [
        {
          "type": "text",
          "value": "n48",
        
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
      "offset": 201,
      "line": 26,
      "column": 9
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
          "offset": 59,
          "line": 5,
          "column": 4
        }
      },
      "value": "szm = true\ngeqxw_cjpyba = 2
```
## 3. [FM-MDAST] (structured)

**Input:** `"---\njwuks: \"6vz 1ln0yjk\"\n---\n\n**n3hlcsdv1**\n\n- k9lcallerknb\n- vx0l4\n- pz1z\n\nb\n\n- s0a\n- 7w\n- pira\n- ug\n- gaa8fj2a\n\n- vqc1\n- jih qefy\n- 59wvgowo94bd\n\n*caller*"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "yaml",
      "value": "jwuks: \"6vz 1ln0yjk\"",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 3,
          "column": 4,
          "offset": 28
        }
      }
    },
    {
      "type": "paragraph",
      "children": [
        {
          "type": "strong",
          "children": [
            {
              "type": "text",
           
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
      "offset": 156,
      "line": 23,
      "column": 9
    }
  },
  "children": [
    {
      "type": "yaml",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 28,
          "line": 3,
          "column": 4
        }
      },
      "value": "jwuks: \"6vz 1ln0yjk\""
    
```
## 4. [FM-MDAST] (structured)

**Input:** `"+++\nzz = false\naijrzvls_k_f = \" \"\nrhv_sosv = \"orhc54xw5\"\nfynoyjvo_ = 2231\nj = gtl\n+++\n\n**dtm**\n\n`hurxxpsizfjn`\n\n- h\n\n- fracs0t\n\n6gk2dgl\n\n| h | apvzxwngje | ourtjqwlimux |\n| --- | --- | --- |\n| kst | a | jwrdrne |\n\n- tn6\n\n`o`"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "toml",
      "value": "zz = false\naijrzvls_k_f = \" \"\nrhv_sosv = \"orhc54xw5\"\nfynoyjvo_ = 2231\nj = gtl",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 7,
          "column": 4,
          "offset": 85
        }
      }
    },
    {
      "type": "paragraph",
      "children": [
        {
          "type": "strong",
          "child
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
      "offset": 224,
      "line": 25,
      "column": 4
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
          "offset": 85,
          "line": 7,
          "column": 4
        }
      },
      "value": "zz = false\naijrzvls_k_f = \
```
## 5. [FM-MDAST] (structured)

**Input:** `"+++\nbmaazzulif = 1172\niieg = hkjienhw\nunha = 3499\ngp = 1729\nf = false\n+++\n\n**b8**\n\n1. asfg7\n2. c5sm5p\n3. lck0\n4. 4q5ahu\n5. k\n\na0 s291\n\n1. han\n2. zfi\n3. ntjkdrmwtc\n4. 2\n5. f5\n\n1. xp7d5qmj4\n\n- c79wn5\n\n> qkm"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "toml",
      "value": "bmaazzulif = 1172\niieg = hkjienhw\nunha = 3499\ngp = 1729\nf = false",
      "position": {
        "start": {
          "line": 1,
          "column": 1,
          "offset": 0
        },
        "end": {
          "line": 7,
          "column": 4,
          "offset": 73
        }
      }
    },
    {
      "type": "paragraph",
      "children": [
        {
          "type": "strong",
          "children": [
        
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
      "offset": 204,
      "line": 29,
      "column": 6
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
          "offset": 73,
          "line": 7,
          "column": 4
        }
      },
      "value": "bmaazzulif = 1172\niieg = hk
```
