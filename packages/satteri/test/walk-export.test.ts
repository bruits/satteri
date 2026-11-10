import { test, expect } from "vitest";
import {
  createMdastHandle,
  dropHandle,
  MdastReader,
  serializeHandle,
  walkMdastHandle,
} from "../src/index.js";
import { NodeType } from "../src/mdast/generated/node-types.js";

test("walkMdastHandle from the root matches the nodes the serialized tree holds", () => {
  const handle = createMdastHandle("# One\n\nTwo\n\n- three\n", undefined, true);
  try {
    const subscriptions = [NodeType.Heading, NodeType.Paragraph, NodeType.List].map((nodeType) => ({
      nodeType,
      tagFilter: [],
    }));
    const matches = walkMdastHandle(handle, subscriptions);
    const view = new DataView(matches.buffer, matches.byteOffset, matches.byteLength);
    const matchedIds = Array.from({ length: view.getUint32(0, true) }, (_, index) =>
      view.getUint32(4 + 10 * index, true),
    );

    const reader = new MdastReader(serializeHandle(handle));
    const expectedIds = [];
    for (let nodeId = 0; nodeId < reader.nodeCount; nodeId += 1) {
      const nodeType = reader.getNodeType(nodeId);
      if (
        nodeType === NodeType.Heading ||
        nodeType === NodeType.Paragraph ||
        nodeType === NodeType.List
      ) {
        expectedIds.push(nodeId);
      }
    }
    expect(matchedIds.toSorted((a, b) => a - b)).toEqual(expectedIds);
    expect(expectedIds.length).toBeGreaterThan(2);
  } finally {
    dropHandle(handle);
  }
});
