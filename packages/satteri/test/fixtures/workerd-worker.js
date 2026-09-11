import {
  defineHastPlugin,
  defineMdastPlugin,
  hastToHtml,
  htmlToHast,
  markdownToHtml,
  markdownToHast,
  markdownToJs,
  markdownToMdast,
  mdxToJs,
} from "satteri";

let requests = 0;

export default {
  async fetch(request) {
    const source = await request.text();
    const mdastPlugin = defineMdastPlugin({
      name: "workerd-mdast",
      async inlineCode(node, ctx) {
        await Promise.resolve();
        ctx.replaceNode(node, { type: "text", value: node.value.toUpperCase() });
      },
    });
    const hastPlugin = defineHastPlugin({
      name: "workerd-hast",
      element: {
        filter: ["h1"],
        async visit(node, ctx) {
          await Promise.resolve();
          ctx.appendChild(node, { type: "text", value: "!" });
        },
      },
    });
    let invalidMdx;
    try {
      mdxToJs("export const =");
    } catch (error) {
      invalidMdx = error.message;
    }

    return Response.json({
      requests: ++requests,
      hasBuffer: "Buffer" in globalThis,
      html: markdownToHtml(source).html,
      plugins: (
        await markdownToHtml("# Hello\n\nUse `code`.", {
          mdastPlugins: [mdastPlugin],
          hastPlugins: [hastPlugin],
        })
      ).html,
      mdast: markdownToMdast("**Hello**"),
      hast: markdownToHast("**Hello**"),
      htmlRoundTrip: hastToHtml(htmlToHast("<p>Hello <em>workerd</em></p>", { fragment: true })),
      markdownJs: markdownToJs("# Hello").code,
      mdxJs: mdxToJs("export const answer = 42\n\n# Hello {answer}\n\n<Widget />").code,
      invalidMdx,
      recovered: markdownToHtml("Recovered").html,
    });
  },
};
