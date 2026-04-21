import { describe, test } from "vitest";
import { assertExtHastConformance } from "./helpers.js";

const ALERTS: ["gfmAlerts"] = ["gfmAlerts"];

describe("GFM alerts HAST conformance", () => {
  test("note alert", () => {
    assertExtHastConformance("> [!NOTE]\n> Note content", ALERTS);
  });

  test("tip alert", () => {
    assertExtHastConformance("> [!TIP]\n> Tip content", ALERTS);
  });

  test("important alert", () => {
    assertExtHastConformance("> [!IMPORTANT]\n> Important content", ALERTS);
  });

  test("warning alert", () => {
    assertExtHastConformance("> [!WARNING]\n> Warning content", ALERTS);
  });

  test("caution alert", () => {
    assertExtHastConformance("> [!CAUTION]\n> Caution content", ALERTS);
  });

  test("alert with multiple paragraphs", () => {
    assertExtHastConformance("> [!NOTE]\n> Line 1.\n>\n> Line 2.", ALERTS);
  });

  test("nested alerts", () => {
    assertExtHastConformance("> [!CAUTION]\n> Line 1.\n>\n> > [!NOTE]\n> > Line 2.", ALERTS);
  });

  test("regular blockquote (not an alert)", () => {
    assertExtHastConformance("> Just a quote", ALERTS);
  });

  test("alert with formatting", () => {
    assertExtHastConformance("> [!TIP]\n> **Bold** and *italic*", ALERTS);
  });
});
