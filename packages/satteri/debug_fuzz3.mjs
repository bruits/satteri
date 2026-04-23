import { markdownToHast } from "./dist/index.js";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";

function normalizeAlignToStyle(node) {
  if (typeof node !== "object" || node === null) return node;
  const out = { ...node };
  delete out.data;
  if (out.properties && typeof out.properties === "object") {
    const props = { ...out.properties };
    if ("align" in props && typeof props.align === "string") {
      props.style = `text-align: ${props.align}`;
      delete props.align;
    }
    out.properties = props;
  }
  if (Array.isArray(out.children)) out.children = out.children.map(normalizeAlignToStyle);
  return out;
}

const proc = unified().use(remarkParse).use(remarkGfm).use(remarkRehype, { allowDangerousHtml: true });

const input = "[2](https://example.com/hvjzgslnzbcy)\n\nx\n\n`f`\n\n- [x] 0x fnjtbl78\n- [x]  \n- [ ] qsodfr3dq\n- [x]  fs2t8cwi \n- [ ] r9siho fpw\n\n[w2](https://example.com/lfpudzc)\n\n- fc82\n- vq08v1glvneg\n- ajrerygal\n- wc4bqmznsrx8\n- sq2rb";

// Run many times to see if output is stable.
const results = new Set();
for (let i = 0; i < 50; i++) {
  const sat = JSON.stringify(markdownToHast(input));
  results.add(sat);
}
console.log(`satteri produced ${results.size} distinct output(s) over 50 calls`);

const remarkOnce = JSON.stringify(normalizeAlignToStyle(JSON.parse(JSON.stringify(proc.runSync(proc.parse(input))))));
const satOnce = [...results][0];
console.log(`remark length: ${remarkOnce.length}`);
console.log(`satteri length: ${satOnce.length}`);
console.log(`equal: ${remarkOnce === satOnce}`);

if (remarkOnce !== satOnce) {
  // Find first differing offset.
  for (let i = 0; i < Math.min(remarkOnce.length, satOnce.length); i++) {
    if (remarkOnce[i] !== satOnce[i]) {
      console.log(`first diff at ${i}:`);
      console.log(`  remark: ...${remarkOnce.slice(Math.max(0, i - 20), i + 80)}...`);
      console.log(`  satteri: ...${satOnce.slice(Math.max(0, i - 20), i + 80)}...`);
      break;
    }
  }
}
