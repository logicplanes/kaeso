export function showCursor() {
  const show = () => {
    const pointer = document.createElement("div");
    pointer.innerHTML =
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M4 3v15.5l4.2-4 3 6.6 2.8-1.2-3-6.5H17z" fill="#111" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>';
    Object.assign(pointer.style, {
      position: "fixed",
      left: "0",
      top: "0",
      zIndex: "2147483647",
      pointerEvents: "none",
      transform: "translate(-40px, -40px)",
      filter: "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.35))",
    });
    const arrow = pointer.firstElementChild;
    Object.assign(arrow.style, { display: "block", transformOrigin: "4px 3px", transition: "transform 120ms ease" });
    document.documentElement.append(pointer);
    addEventListener("mousemove", (e) => (pointer.style.transform = `translate(${e.clientX - 4}px, ${e.clientY - 3}px)`), true);
    addEventListener("mousedown", () => (arrow.style.transform = "scale(0.8)"), true);
    addEventListener("mouseup", () => (arrow.style.transform = ""), true);
  };
  if (document.readyState === "loading") addEventListener("DOMContentLoaded", show);
  else show();
}

export function openTerminal() {
  const style = document.createElement("style");
  style.textContent = `
    #demo-terminal { position: fixed; left: 272px; right: 32px; bottom: 32px; z-index: 2147483646; border-radius: 12px; background: #111214; color: #e8e8ea;
      box-shadow: 0 24px 48px -12px rgba(0, 0, 0, 0.45), 0 0 0 1px rgba(255, 255, 255, 0.08); font: 13px/20px ui-monospace, "SF Mono", Menlo, monospace;
      opacity: 0; transform: translateY(16px); transition: opacity 0.25s ease, transform 0.25s ease; }
    #demo-terminal.open { opacity: 1; transform: none; }
    #demo-terminal .bar { display: flex; gap: 8px; padding: 12px 14px 0; }
    #demo-terminal .bar i { width: 11px; height: 11px; border-radius: 50%; background: #3a3b3f; }
    #demo-terminal pre { margin: 0; padding: 10px 18px 16px; min-height: 200px; white-space: pre-wrap; font: inherit; }
    #demo-terminal .dim { color: #8b8d93; }`;
  const panel = document.createElement("div");
  panel.id = "demo-terminal";
  panel.innerHTML = '<div class="bar"><i></i><i></i><i></i></div><pre></pre>';
  document.documentElement.append(style, panel);
  requestAnimationFrame(() => requestAnimationFrame(() => panel.classList.add("open")));
}

export async function writeTerminal({ text, dim, delay }) {
  const span = document.createElement("span");
  if (dim) span.className = "dim";
  document.querySelector("#demo-terminal pre").append(span);
  if (!delay) return void (span.textContent = text);
  for (const ch of text) {
    span.textContent += ch;
    await new Promise((resolve) => setTimeout(resolve, delay));
  }
}
