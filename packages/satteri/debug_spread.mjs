import { markdownToMdast } from "./dist/index.js";
import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";

const processor = unified().use(remarkParse).use(remarkGfm);

const input = "gpr\n\n- e4smu\n- 245t2hw\n\n  m27rz3ex9";

const expected = processor.parse(input);
const list = expected.children.find(n => n.type === 'list');

console.log("=== Expected Structure ===");
console.log("List spread:", list.spread);
console.log("List has", list.children.length, "items\n");

list.children.forEach((item, i) => {
  console.log(`ListItem ${i}:`);
  console.log("  spread:", item.spread);
  console.log("  children:", item.children.map(c => c.type));
  if (item.children.length > 1) {
    console.log("  ^ Multiple children means SPREAD=true for this item");
  }
});

const actual = markdownToMdast(input);
const listA = actual.children.find(n => n.type === 'list');

console.log("\n=== Actual Structure ===");
console.log("List spread:", listA.spread);
console.log("List has", listA.children.length, "items\n");

listA.children.forEach((item, i) => {
  console.log(`ListItem ${i}:`);
  console.log("  spread:", item.spread);
  console.log("  children:", item.children.map(c => c.type));
  if (item.children.length > 1) {
    console.log("  ^ Multiple children means SPREAD=true for this item");
  }
});
