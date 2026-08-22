//   node scripts/binary-size.mjs measure <binary> [--out file.json]
//   node scripts/binary-size.mjs compare <base.json> <head.json>
//     [--max-growth 5] [--comment-threshold 1] [--status status.json]

import { execFileSync } from "node:child_process";
import { readFileSync, statSync, writeFileSync } from "node:fs";

// Lets the workflow update its own comment rather than post one per push.
const COMMENT_MARKER = "<!-- binary-size -->";

// `.rela.dyn` and `.data.rel.ro` grow with static pointer tables, not with code.
const TRACKED_SECTIONS = [".text", ".rodata", ".rela.dyn", ".data.rel.ro", ".eh_frame"];

function sections(binary) {
  let output;
  try {
    output = execFileSync("size", ["-A", "-d", binary], { encoding: "utf8" });
  } catch {
    return null;
  }
  const found = {};
  for (const line of output.split("\n")) {
    const [name, size] = line.trim().split(/\s+/);
    if (TRACKED_SECTIONS.includes(name)) found[name] = Number(size);
  }
  return found;
}

function measure(binary) {
  return { binary, total: statSync(binary).size, sections: sections(binary) };
}

function bytes(n) {
  const sign = n < 0 ? "-" : "";
  const abs = Math.abs(n);
  if (abs < 1024) return `${sign}${abs} B`;
  return `${sign}${(abs / 1024).toFixed(1)} KiB`;
}

function delta(before, after) {
  const diff = after - before;
  if (diff === 0) return "no change";
  const percent = before === 0 ? 0 : (diff / before) * 100;
  return `${diff > 0 ? "+" : ""}${bytes(diff)} (${diff > 0 ? "+" : ""}${percent.toFixed(2)}%)`;
}

function compare(base, head, maxGrowth) {
  const grew = ((head.total - base.total) / base.total) * 100;
  const lines = [
    COMMENT_MARKER,
    "## Native binary size",
    "",
    "| | Base | This PR | Change |",
    "|---|---:|---:|---|",
    `| **Total** | ${bytes(base.total)} | ${bytes(head.total)} | ${delta(base.total, head.total)} |`,
  ];

  if (base.sections && head.sections) {
    for (const name of TRACKED_SECTIONS) {
      const before = base.sections[name];
      const after = head.sections[name];
      if (before === undefined || after === undefined || before === after) continue;
      lines.push(`| \`${name}\` | ${bytes(before)} | ${bytes(after)} | ${delta(before, after)} |`);
    }
  }

  lines.push("");
  const over = grew > maxGrowth;
  lines.push(
    over
      ? `Grew by ${grew.toFixed(2)}%, over the ${maxGrowth}% budget. Raise \`--max-growth\` in \`.github/workflows/binary-size.yml\` if this is expected.`
      : `Within the ${maxGrowth}% growth budget.`,
  );
  return { report: lines.join("\n"), over, percent: grew };
}

function flag(args, name, fallback) {
  const at = args.indexOf(name);
  return at === -1 ? fallback : args[at + 1];
}

const [command, ...args] = process.argv.slice(2);

if (command === "measure") {
  const result = measure(args[0]);
  const out = flag(args, "--out", null);
  const json = JSON.stringify(result, null, 2);
  if (out) writeFileSync(out, json);
  else console.log(json);
} else if (command === "compare") {
  const base = JSON.parse(readFileSync(args[0], "utf8"));
  const head = JSON.parse(readFileSync(args[1], "utf8"));
  const { report, over, percent } = compare(base, head, Number(flag(args, "--max-growth", 5)));
  console.log(report);

  const status = flag(args, "--status", null);
  if (status) {
    const threshold = Number(flag(args, "--comment-threshold", 1));
    writeFileSync(
      status,
      JSON.stringify({ significant: Math.abs(percent) >= threshold, over, percent }),
    );
  }
  if (over) process.exitCode = 1;
} else {
  console.error("usage: binary-size.mjs measure <binary> | compare <base.json> <head.json>");
  process.exitCode = 2;
}
