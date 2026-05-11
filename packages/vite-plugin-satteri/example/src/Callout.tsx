import type { ComponentChildren } from "preact";
import { useState } from "preact/hooks";

export function Callout({
  tone = "info",
  children,
}: {
  tone?: "info" | "warning";
  children: ComponentChildren;
}) {
  const [count, setCount] = useState(0);
  const background = tone === "warning" ? "#fff8e1" : "#e7f1ff";
  const border = tone === "warning" ? "#f0c244" : "#2f6feb";

  return (
    <div
      style={{
        padding: "0.75rem 1rem",
        background,
        border: `1px solid ${border}`,
        borderRadius: 6,
      }}
    >
      <div>{children}</div>
      <button
        type="button"
        onClick={() => setCount((n) => n + 1)}
        style={{
          marginTop: "0.5rem",
          padding: "0.3rem 0.7rem",
          background: border,
          color: "#fff",
          border: "none",
          borderRadius: 4,
          cursor: "pointer",
        }}
      >
        Clicked {count} {count === 1 ? "time" : "times"}
      </button>
    </div>
  );
}
