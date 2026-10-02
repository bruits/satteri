import { markdownToMdast, mdxToMdast } from "../src/index.js";
import type { Root } from "mdast";

const markdownRoot: Root = markdownToMdast("# Hello");
const mdxRoot: Root = mdxToMdast("# Hello");

void markdownRoot;
void mdxRoot;
