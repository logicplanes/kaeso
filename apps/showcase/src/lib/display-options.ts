import { useState } from "react";
import { icons } from "@logicplanes/kaeso-ui";

export function useLogDisplayOptions() {
  const [view, setView] = useState("list");
  const [group, setGroup] = useState("status");
  const [order, setOrder] = useState("time");
  const [empty, setEmpty] = useState(false);
  const [props, setProps] = useState(["model", "latency", "tokens", "cost"]);
  return {
    view: {
      value: view,
      onChange: setView,
      options: [
        { value: "list", label: "List", icon: icons.list },
        { value: "board", label: "Board", icon: icons.board },
      ],
    },
    rows: [
      {
        label: "Grouping",
        icon: icons.stack,
        value: group,
        onChange: setGroup,
        options: [
          { value: "status", label: "Status" },
          { value: "model", label: "Model" },
          { value: "key", label: "Key" },
          { value: "none", label: "No grouping" },
        ],
      },
      {
        label: "Ordering",
        icon: icons.sort,
        value: order,
        onChange: setOrder,
        options: [
          { value: "time", label: "Newest" },
          { value: "latency", label: "Slowest" },
          { value: "cost", label: "Most expensive" },
        ],
      },
    ],
    toggles: [{ label: "Show empty groups", checked: empty, onChange: setEmpty }],
    properties: {
      value: props,
      onChange: setProps,
      options: ["model", "key", "status", "latency", "tokens", "cost", "provider"].map((p) => ({ value: p, label: p[0]!.toUpperCase() + p.slice(1) })),
    },
  };
}
