import { describe, test, expect } from "vitest";
import { markdownToHtml, mdxToJs } from "../../src/index.js";
import { createMdastHandle, getHandleSource } from "../../index.js";
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

describe("definition list: disabled by default (never break userspace)", () => {
  test("a `: ` line stays a paragraph when the feature is off", () => {
    const out = html("Apple\n:   Red.\n", {});
    expect(out).not.toContain("<dl>");
    expect(out).not.toContain("<dd>");
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
});
