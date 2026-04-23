# Fuzz-discovered conformance issues

Found 4 unique issue(s) across 4 total failure(s).

## 1. [HAST] (structured)

**Input:** `"---\n\n1. 0dluxxtlviv\n\n```rust\ng4967\n```\n\n#### 2w0rp1bf kjy\n\n# cctv08o1\n\nihrmu12tz\n\n1kwvpg8f\n\n*ixd*"`

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
      "tagName": "p",
      "properties": {},
      "c
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
      "offset": 97,
      "line": 17,
      "column": 6
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
          "line": 1,
          "column": 4
        }
      },
      "tagName": "hr",
      "properties": 
```
## 2. [HTML] (structured)

**Input:** `"---\n\n[r](https://example.com/avj)\n\ncz0vv4bsj\n\n#### ca\n\n**f**\n\n```ts\nm\n```\n\n- [ ] 7z\n- [x] pc5\n- [ ] 1v nvh9wj4xi\n- [x] 1\n- [ ] x0u\n\n| cqfzxto | xgyxoaujpwli |\n| --- | --- |\n| xxrwu | tfzce |\n\nlu22eq\n\n```js\n;\n```\n\n1. xs 07uvva\n2. sj10p\n3. 0untpm\n\n**m**"`

**Expected (reference):**
```json
"<hr />\n<p><a href=\"https://example.com/avj\">r</a></p>\n<p>cz0vv4bsj</p>\n<h4>ca</h4>\n<p><strong>f</strong></p>\n<pre><code class=\"language-ts\">m\n</code></pre>\n<p>- [ ] 7z\n- [x] pc5\n- [ ] 1v nvh9wj4xi\n- [x] 1\n- [ ] x0u</p>\n<table>\n<thead>\n<tr>\n<th>cqfzxto</th>\n<th>xgyxoaujpwli</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>xxrwu</td>\n<td>tfzce</td>\n</tr>\n</tbody>\n</table>\n<p>lu22eq</p>\n<pre><code class=\"language-js\">;\n</code></pre>\n<p>1. xs 07uvva\n2. sj10p\n3. 0untpm</p>\n
```

**Actual (Sätteri):**
```json
"<hr />\n<p><a href=\"https://example.com/avj\">r</a></p>\n<p>cz0vv4bsj</p>\n<h4>ca</h4>\n<p><strong>f</strong></p>\n<pre><code class=\"language-ts\">m\n</code></pre>\n<ul class=\"contains-task-list\">\n<li class=\"task-list-item\"><input type=\"checkbox\" disabled> 7z</li>\n<li class=\"task-list-item\"><input type=\"checkbox\" checked disabled> pc5</li>\n<li class=\"task-list-item\"><input type=\"checkbox\" disabled> 1v nvh9wj4xi</li>\n<li class=\"task-list-item\"><input type=\"checkbox\" check
```
## 3. [HTML] (structured)

**Input:** `"---\n\nsko\n\n> k32x9pu\n\nm5hxwo\n\nj \n\n![fhya](https://example.com/ynbhxnbgz)"`

**Expected (reference):**
```json
"<hr />\n<p>sko</p>\n<p>> k32x9pu</p>\n<p>m5hxwo</p>\n<p>j</p>\n<p><img src=\"https://example.com/ynbhxnbgz\" alt=\"fhya\"></p>"
```

**Actual (Sätteri):**
```json
"<hr />\n<p>sko</p>\n<blockquote>\n<p>k32x9pu</p>\n</blockquote>\n<p>m5hxwo</p>\n<p>j</p>\n<p><img src=\"https://example.com/ynbhxnbgz\" alt=\"fhya\"></p>"
```
## 4. [HTML] (structured)

**Input:** `"---\n\n`pe`\n\n*2 d*\n\n1. krrj9\n\n- 4p70otg1s\n- ku1k86\n- zim \n\n1. au0lazk \n2. eap52onr\n3. tgb5\n4. eqr\n\n| lffqkexujl | wjzqplvnzv | rimue |\n| --- | --- | --- |\n| eczvkt | ekwlbuvsqvj | phfxktkrh |\n\n1. yoyut\n2. 5r4ld3u\n3. 41z70\n4. y4bfdtw\n5. 1s7z4m6y\n\n![pywirov](https://example.com/ngakdosk)"`

**Expected (reference):**
```json
"<hr />\n<p><code>pe</code></p>\n<p><em>2 d</em></p>\n<p>1. krrj9</p>\n<p>- 4p70otg1s\n- ku1k86\n- zim</p>\n<p>1. au0lazk\n2. eap52onr\n3. tgb5\n4. eqr</p>\n<table>\n<thead>\n<tr>\n<th>lffqkexujl</th>\n<th>wjzqplvnzv</th>\n<th>rimue</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>eczvkt</td>\n<td>ekwlbuvsqvj</td>\n<td>phfxktkrh</td>\n</tr>\n</tbody>\n</table>\n<p>1. yoyut\n2. 5r4ld3u\n3. 41z70\n4. y4bfdtw\n5. 1s7z4m6y</p>\n<p><img src=\"https://example.com/ngakdosk\" alt=\"pywirov\"></p>"
```

**Actual (Sätteri):**
```json
"<hr />\n<p><code>pe</code></p>\n<p><em>2 d</em></p>\n<ol>\n<li>krrj9</li>\n</ol>\n<ul>\n<li>4p70otg1s</li>\n<li>ku1k86</li>\n<li>zim</li>\n</ul>\n<ol>\n<li>au0lazk</li>\n<li>eap52onr</li>\n<li>tgb5</li>\n<li>eqr</li>\n</ol>\n<table>\n<thead>\n<tr>\n<th>lffqkexujl</th>\n<th>wjzqplvnzv</th>\n<th>rimue</th>\n</tr>\n</thead>\n<tbody>\n<tr>\n<td>eczvkt</td>\n<td>ekwlbuvsqvj</td>\n<td>phfxktkrh</td>\n</tr>\n</tbody>\n</table>\n<ol>\n<li>yoyut</li>\n<li>5r4ld3u</li>\n<li>41z70</li>\n<li>y4bfdtw</li>\n
```
