import { markdownToMdast } from "./dist/index.js";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";

const processor = unified().use(remarkParse).use(remarkGfm);

const input = "gpr\n\n- e4smu\n- 245t2hw\n\n  m27rz3ex9";
console.log("=== Input ===");
console.log(JSON.stringify(input));
console.log("\n=== Expected (remark) ===");
const expected = processor.parse(input);
const list = expected.children.find(n => n.type === 'list');
console.log("List spread:", list.spread);
console.log("ListItem 0 spread:", list.children[0].spread);
console.log("ListItem 1 spread:", list.children[1].spread);

console.log("\n=== Actual (satteri) ===");
const actual = markdownToMdast(input);
const listA = actual.children.find(n => n.type === 'list');
console.log("List spread:", listA.spread);
console.log("ListItem 0 spread:", listA.children[0].spread);
console.log("ListItem 1 spread:", listA.children[1].spread);
