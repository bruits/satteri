import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../../src/index.js";
import {
  createMdastHandle,
  getHandleSource,
  applyCommandsAndConvertToHastHandle,
  renderHandle,
} from "../../index.js";
import { visitMdastHandle, resolveMdastSubscriptions } from "../../src/mdast/mdast-visitor.js";
import { defineMdastPlugin } from "../../src/plugin.js";

// Definition lists aren't in the remark/rehype reference pipeline, so we can't
// use the remark conformance harness here — assert HTML/mdast shape directly.

function html(md: string, features: Record<string, unknown> = { definitionList: true }): string {
  const r = markdownToHtml(md, { features });
  if (r instanceof Promise) throw new Error("expected sync");
  return r.html;
}

describe("definition list: HTML", () => {
  test("tight definition renders <dl>/<dt>/<dd> without a nested <p>", () => {
    const out = html("Apple\n:   Red.\n");
    expect(out).toContain("<dt>Apple</dt>");
    expect(out).toContain("<dd>Red.</dd>");
  });

  test("loose definition (blank line) wraps the <dd> content in a <p>", () => {
    expect(html("Apple\n\n:   Red.\n")).toContain("<dd>\n<p>Red.</p>\n</dd>");
  });

  test("one term can have multiple definitions", () => {
    const out = html("Apple\n:   Red.\n:   Green.\n");
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).toContain("<dd>Green.</dd>");
  });

  test("inline markup is parsed in both term and definition", () => {
    const out = html("*Apple*\n:   A **fruit**.\n");
    expect(out).toContain("<dt><em>Apple</em></dt>");
    expect(out).toContain("<strong>fruit</strong>");
  });
});

describe("definition list: per-dd tight/loose (matches mdast-util-definition-list)", () => {
  // Each definition is loose (its <dd> keeps an inner <p>) iff a blank line
  // directly precedes its own `:` marker — the same per-dd rule as
  // mdast-util-definition-list, verified against that library.

  test("blank before the 2nd definition makes only the 2nd loose", () => {
    const out = html("Apple\n:   Red.\n\n:   Green.\n");
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).toContain("<dd>\n<p>Green.</p>\n</dd>");
  });

  test("blank between term and 1st definition makes only the 1st loose", () => {
    const out = html("Apple\n\n:   Red.\n:   Green.\n");
    expect(out).toContain("<dd>\n<p>Red.</p>\n</dd>");
    expect(out).toContain("<dd>Green.</dd>");
  });

  test("a blank before the 3rd definition leaves the first two tight", () => {
    const out = html("Apple\n:   Red.\n:   Green.\n\n:   Blue.\n");
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).toContain("<dd>Green.</dd>");
    expect(out).toContain("<dd>\n<p>Blue.</p>\n</dd>");
  });

  test("a fully tight list unwraps every <dd>", () => {
    const out = html("Apple\n:   Red.\n:   Green.\n");
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).toContain("<dd>Green.</dd>");
    expect(out).not.toContain("<p>");
  });

  test("a fully loose list wraps every <dd>", () => {
    const out = html("Apple\n\n:   Red.\n\n:   Green.\n");
    expect(out).toContain("<dd>\n<p>Red.</p>\n</dd>");
    expect(out).toContain("<dd>\n<p>Green.</p>\n</dd>");
  });
});

describe("definition list: block content, nesting & positions", () => {
  function run(md: string, plugin: Parameters<typeof resolveMdastSubscriptions>[0]) {
    const handle = createMdastHandle(md, { definitionList: true });
    const source = getHandleSource(handle);
    const subs = resolveMdastSubscriptions(plugin);
    visitMdastHandle(handle, plugin, subs, source, undefined);
  }

  test("a loose <dd> exposes multiple block (paragraph) children", () => {
    let types: string[] = [];
    run(
      "Apple\n\n:   Red.\n\n    More red.\n",
      defineMdastPlugin({
        name: "dd-blocks",
        descriptionDetails(n) {
          types = n.children.map((c) => c.type);
        },
      }),
    );
    expect(types).toEqual(["paragraph", "paragraph"]);
  });

  test("a <dd> can contain a nested list", () => {
    let types: string[] = [];
    run(
      "Apple\n\n:   item:\n\n    - a\n    - b\n",
      defineMdastPlugin({
        name: "dd-list",
        descriptionDetails(n) {
          types = n.children.map((c) => c.type);
        },
      }),
    );
    expect(types).toEqual(["paragraph", "list"]);
  });

  test("several term lines become separate <dt> / descriptionTerm nodes", () => {
    // Matches mdast-util-definition-list: each term line is its own term.
    const out = html("Term 1\nTerm 2\n:   Shared.\n");
    expect(out).toContain("<dt>Term 1</dt>");
    expect(out).toContain("<dt>Term 2</dt>");
    expect(out).not.toContain("<dt>Term 1\nTerm 2</dt>");

    const childTypes: string[] = [];
    const termText: string[] = [];
    run(
      "Term 1\nTerm 2\n:   Shared.\n",
      defineMdastPlugin({
        name: "multi-term",
        descriptionList(n) {
          childTypes.push(...n.children.map((c) => c.type));
        },
        descriptionTerm(n) {
          const t = n.children[0];
          if (t?.type === "text") termText.push(t.value);
        },
      }),
    );
    expect(childTypes).toEqual(["descriptionTerm", "descriptionTerm", "descriptionDetails"]);
    expect(termText).toEqual(["Term 1", "Term 2"]);
  });

  test("inline markup resolves independently within each split term", () => {
    const out = html("*A*\nB\n:   Def.\n");
    expect(out).toContain("<dt><em>A</em></dt>");
    expect(out).toContain("<dt>B</dt>");
  });

  test("a definition list nested inside a <dd> is visited", () => {
    let dls = 0;
    const terms: string[] = [];
    run(
      "Outer\n\n:   Inner\n    :   Deep.\n",
      defineMdastPlugin({
        name: "nested",
        descriptionList() {
          dls++;
        },
        descriptionTerm(n) {
          const t = n.children[0];
          if (t?.type === "text") terms.push(t.value);
        },
      }),
    );
    expect(dls).toBe(2); // outer + nested
    expect(terms).toEqual(["Outer", "Inner"]);
  });

  test("dl/dt/dd carry source spans (start and end, line and column)", () => {
    const pos: Record<string, MdastNode["position"]> = {};
    run(
      "Apple\n:   Red.\n",
      defineMdastPlugin({
        name: "pos",
        descriptionList(n) {
          pos.dl = n.position;
        },
        descriptionTerm(n) {
          pos.dt = n.position;
        },
        descriptionDetails(n) {
          pos.dd = n.position;
        },
      }),
    );
    // dt is the term on line 1; dd is the definition on line 2; dl spans both.
    expect(pos.dt?.start).toMatchObject({ line: 1, column: 1 });
    expect(pos.dt?.end.line).toBe(1);
    expect(pos.dd?.start.line).toBe(2);
    expect(pos.dd?.end.line).toBe(2);
    expect(pos.dd!.end.column).toBeGreaterThan(pos.dd!.start.column);
    expect(pos.dl?.start).toMatchObject({ line: 1, column: 1 });
    expect(pos.dl?.end.line).toBe(2);
    // The dl's end offset must reach at least its last child's end.
    expect(pos.dl!.end.offset).toBeGreaterThanOrEqual(pos.dd!.end.offset!);
  });

  test("a plugin can flip a <dd> tight→loose via setProperty(spread)", () => {
    const handle = createMdastHandle("Apple\n:   Red.\n", { definitionList: true });
    const source = getHandleSource(handle);
    const plugin = defineMdastPlugin({
      name: "make-loose",
      descriptionDetails(node, context) {
        context.setProperty(node, "spread", true);
      },
    });
    const subs = resolveMdastSubscriptions(plugin);
    const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
      commandBuffer: Uint8Array;
    };
    const hast = applyCommandsAndConvertToHastHandle(handle, result.commandBuffer);
    // Tight by default (<dd>Red.</dd>); the mutation makes it loose (wrapped <p>).
    expect(renderHandle(hast)).toContain("<dd>\n<p>Red.</p>\n</dd>");
  });

  test("a plugin can flip a <dd> loose→tight via setProperty(spread)", () => {
    const handle = createMdastHandle("Apple\n\n:   Red.\n", { definitionList: true });
    const source = getHandleSource(handle);
    const plugin = defineMdastPlugin({
      name: "make-tight",
      descriptionDetails(node, context) {
        context.setProperty(node, "spread", false);
      },
    });
    const subs = resolveMdastSubscriptions(plugin);
    const result = visitMdastHandle(handle, plugin, subs, source, undefined) as {
      commandBuffer: Uint8Array;
    };
    const hast = applyCommandsAndConvertToHastHandle(handle, result.commandBuffer);
    const out = renderHandle(hast);
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).not.toContain("<p>");
  });
});

describe("definition list: disabled by default (never break userspace)", () => {
  test("a `: ` line stays a paragraph when the feature is off", () => {
    const out = html("Apple\n:   Red.\n", {});
    expect(out).not.toContain("<dl>");
    expect(out).not.toContain("<dd>");
  });
});

describe("definition list: the marker requires whitespace (matches reference)", () => {
  // pandoc / mdast-util-definition-list require whitespace after the colon.
  // A colon glued to content is not a marker — the line stays a paragraph.
  test("a colon glued to content (`:tada:`) is not a marker", () => {
    const out = html("Apple\n:tada:\n");
    expect(out).not.toContain("<dl>");
    expect(out).toContain("<p>Apple\n:tada:</p>");
  });

  test("a run of colons (`::tada:`) is not a marker", () => {
    const out = html("Apple\n::tada:\n");
    expect(out).not.toContain("<dl>");
    expect(out).toContain("<p>Apple\n::tada:</p>");
  });

  test("a colon followed by a space (`: :tada:`) is a marker", () => {
    const out = html("Apple\n: :tada:\n");
    expect(out).toContain("<dt>Apple</dt>");
    expect(out).toContain("<dd>:tada:</dd>");
  });

  test("a colon followed by a tab is a marker", () => {
    const out = html("Apple\n:\tRed.\n");
    expect(out).toContain("<dd>Red.</dd>");
  });

  test("a lone colon at end of line is an empty definition", () => {
    const out = html("Apple\n:\n");
    expect(out).toContain("<dt>Apple</dt>");
    expect(out).toContain("<dd></dd>");
  });
});

describe("definition list: term–definition association (matches reference)", () => {
  // A term may be one blank line from its definition (loose); two or more
  // blank lines disconnect them, matching pandoc / mdast-util-definition-list.
  test("one blank line keeps the term (loose definition)", () => {
    const out = html("Apple\n\n:   Red.\n");
    expect(out).toContain("<dt>Apple</dt>");
    expect(out).toContain("<dd>\n<p>Red.</p>\n</dd>");
  });

  test("two blank lines disconnect the term — no definition list", () => {
    const out = html("Apple\n\n\n:   Red.\n");
    expect(out).not.toContain("<dl>");
    expect(out).toContain("<p>Apple</p>");
    expect(out).toContain("<p>:   Red.</p>");
  });
});

describe("definition list: directive coexistence", () => {
  test("a `:::` fence is not swallowed into a <dd>", () => {
    const out = html("Apple\n:   Red.\n\n:::note\nbody\n:::\n", {
      definitionList: true,
      directive: true,
    });
    expect(out).toContain("<dl>");
    expect(out).toContain("<dd>Red.</dd>");
    expect(out).not.toContain("<dd>:");
  });

  test("directive output is identical with and without the deflist extension", () => {
    const doc = ":::note\nbody\n:::\n";
    expect(html(doc, { directive: true, definitionList: true })).toBe(
      html(doc, { directive: true }),
    );
  });
});

describe("definition list: mdast visitor + spread", () => {
  function collect(md: string) {
    const handle = createMdastHandle(md, { definitionList: true });
    const source = getHandleSource(handle);
    const seen: string[] = [];
    let ddSpread: boolean | undefined;
    const plugin = defineMdastPlugin({
      name: "collect-deflist",
      descriptionList() {
        seen.push("descriptionList");
      },
      descriptionTerm() {
        seen.push("descriptionTerm");
      },
      descriptionDetails(node) {
        seen.push("descriptionDetails");
        ddSpread = node.spread;
      },
    });
    const subs = resolveMdastSubscriptions(plugin);
    visitMdastHandle(handle, plugin, subs, source, undefined);
    return { seen, ddSpread };
  }

  function runVisitor(md: string, plugin: Parameters<typeof resolveMdastSubscriptions>[0]) {
    const handle = createMdastHandle(md, { definitionList: true });
    const source = getHandleSource(handle);
    const subs = resolveMdastSubscriptions(plugin);
    visitMdastHandle(handle, plugin, subs, source, undefined);
  }

  test("visitors fire for descriptionList/Term/Details in tree order", () => {
    expect(collect("Apple\n:   Red.\n").seen).toEqual([
      "descriptionList",
      "descriptionTerm",
      "descriptionDetails",
    ]);
  });

  test("tight definitionDetails exposes spread=false", () => {
    expect(collect("Apple\n:   Red.\n").ddSpread).toBe(false);
  });

  test("loose definitionDetails exposes spread=true", () => {
    expect(collect("Apple\n\n:   Red.\n").ddSpread).toBe(true);
  });

  test("a plugin reads the descriptionList's dt/dd children", () => {
    let childTypes: string[] = [];
    runVisitor(
      "Apple\n:   Red.\n",
      defineMdastPlugin({
        name: "read-dl",
        descriptionList(node) {
          childTypes = node.children.map((c) => c.type);
        },
      }),
    );
    expect(childTypes).toEqual(["descriptionTerm", "descriptionDetails"]);
  });

  test("a plugin reads the descriptionTerm's inline text", () => {
    let termText: string | undefined;
    runVisitor(
      "Apple\n:   Red.\n",
      defineMdastPlugin({
        name: "read-dt",
        descriptionTerm(node) {
          const first = node.children[0];
          if (first?.type === "text") termText = first.value;
        },
      }),
    );
    expect(termText).toBe("Apple");
  });

  test("a plugin reads the descriptionDetails' block children", () => {
    let ddChildTypes: string[] = [];
    runVisitor(
      "Apple\n:   Red.\n",
      defineMdastPlugin({
        name: "read-dd",
        descriptionDetails(node) {
          ddChildTypes = node.children.map((c) => c.type);
        },
      }),
    );
    expect(ddChildTypes).toEqual(["paragraph"]);
  });
});

describe("definition list: MDX", () => {
  test("a definition list compiles under MDX and coexists with JSX", () => {
    const r = mdxToJs("<Foo />\n\nApple\n:   Red.\n", {
      features: { definitionList: true },
    });
    if (r instanceof Promise) throw new Error("expected sync");
    // The `<dl>` reaches the compiled component and the JSX tag survives.
    expect(r.code).toContain('"dl"');
    expect(r.code).toContain("Foo");
  });

  function mdxCode(md: string): string {
    const r = mdxToJs(md, { features: { definitionList: true } });
    if (r instanceof Promise) throw new Error("expected sync");
    return r.code;
  }

  test("a tight definition compiles with no <p> inside the <dd>", () => {
    const code = mdxCode("Apple\n:   Red.\n");
    expect(code).toContain('"dl"');
    expect(code).toContain('"dt"');
    expect(code).toContain('"dd"');
    expect(code).not.toContain('"p"');
  });

  test("a loose definition compiles with a <p> inside the <dd>", () => {
    const code = mdxCode("Apple\n\n:   Red.\n");
    expect(code).toContain('"dd"');
    expect(code).toContain('"p"');
  });

  test("a definition list nested in a <dd> compiles under MDX", () => {
    const code = mdxCode("Outer\n\n:   Inner\n    :   Deep.\n");
    // The nested structure survives compilation: dl/dt/dd tags and the inner
    // term/definition text all reach the compiled output.
    expect(code).toContain('"dl"');
    expect(code).toContain('"dd"');
    expect(code).toContain("Outer");
    expect(code).toContain("Inner");
    expect(code).toContain("Deep.");
  });
});
