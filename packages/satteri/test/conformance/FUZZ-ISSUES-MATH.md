# Math fuzz-discovered conformance issues

Found 2 unique issue(s) across 2 total failure(s).

## 1. [MATH-MDAST] (structured)

**Input:** `"| ziaqnuv | qzkmvtgdh | l | ohzyy |\n| --- | --- | --- | --- |\n| skfgo | omlal | me | uzahuieojqsw |\n| cluijioaff | tilkciig |  |  |\n| vrvy | ldupam | kctmvpoeylv |  |\n\n- eb 2yamgs211\n- pk\n- duomsqyb2p\n- 7g8wi\n- 8srla72pra\n\n- pva42s4 \n- b2f i6r\n\n*2ds*\n\n---\n\n2 84xgk19"`

**Expected (reference):**
```json
{
  "type": "root",
  "children": [
    {
      "type": "table",
      "align": [
        null,
        null,
        null,
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
                  "value": "ziaqnuv",
                  "position": {
                    "start": {
                      "line": 1,
       
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
      "offset": 266,
      "line": 20,
      "column": 10
    }
  },
  "children": [
    {
      "type": "table",
      "position": {
        "start": {
          "offset": 0,
          "line": 1,
          "column": 1
        },
        "end": {
          "offset": 166,
          "line": 5,
          "column": 35
        }
      },
      "align": [
        null,
        n
```
## 2. [MATH-MDAST] (structured)

**Input:** `"75m4b\n\n- rb \n- v0cogsu0cr\n- 97hrbe7o\n- 6j\n- 4djxpo n\n\n- i57zsvubq\n\n## suhae\n\n`dpyqovtebfjq`\n\n$i96krzlr$\n\n**d9fp**"`

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
          "value": "75m4b",
          "position": {
            "start": {
              "line": 1,
              "column": 1,
              "offset": 0
            },
            "end": {
              "line": 1,
              "column": 6,
              "offset": 5
            }
          }
        }
      ],
      "position": {
        "start": {
          "line": 1,
   
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
      "offset": 113,
      "line": 17,
      "column": 9
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
          "offset": 5,
          "line": 1,
          "column": 6
        }
      },
      "children": [
        {
          
```
