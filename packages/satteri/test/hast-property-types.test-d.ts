import { defineHastPlugin } from "../src/index.js";
import type { HastNode } from "../src/index.js";

type Expect<T extends true> = T;
type Equals<A, B> =
  (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;

type Properties = Extract<HastNode, { type: "element" }>["properties"];

// A blanket `PropertyValue` index signature widens all of these to one union.
export type _Href = Expect<Equals<Properties["href"], string | undefined>>;
export type _ClassName = Expect<Equals<Properties["className"], Array<string> | undefined>>;
export type _TabIndex = Expect<Equals<Properties["tabIndex"], number | string | undefined>>;
export type _Start = Expect<Equals<Properties["start"], number | string | undefined>>;
export type _Disabled = Expect<Equals<Properties["disabled"], boolean | string | undefined>>;

const observed: Array<string | number | boolean | undefined> = [];

export const readsPropertiesUnguarded = defineHastPlugin({
  name: "reads-properties-unguarded",
  element: {
    filter: ["a", "ol"],
    visit(node) {
      const external: boolean = node.properties.href?.startsWith("http") ?? false;
      const classes: string = node.properties.className?.join(" ") ?? "";
      const firstClass: string | undefined = node.properties.className?.[0];
      const start: number | string | undefined = node.properties.start;
      observed.push(external, classes, firstClass, start);
    },
  },
});
