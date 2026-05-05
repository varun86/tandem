const FULL_CALENDAR_STYLE_SELECTOR = "style[data-fullcalendar]";
const MAX_STYLE_SHEET_WAIT_FRAMES = 8;

function nextFrame() {
  return new Promise<void>((resolve) => {
    if (typeof window !== "undefined" && typeof window.requestAnimationFrame === "function") {
      window.requestAnimationFrame(() => resolve());
      return;
    }
    setTimeout(resolve, 0);
  });
}

export async function prepareFullCalendarStyleSheet() {
  if (typeof document === "undefined") return;

  const parent = document.head || document.documentElement;
  if (!parent) return;

  let style = document.querySelector<HTMLStyleElement>(FULL_CALENDAR_STYLE_SELECTOR);
  if (!style) {
    style = document.createElement("style");
    style.setAttribute("data-fullcalendar", "");
    parent.appendChild(style);
  } else if (!style.isConnected) {
    parent.appendChild(style);
  }

  // FullCalendar injects rules at module evaluation time and assumes
  // HTMLStyleElement.sheet is immediately available. Tauri/WebKit can expose
  // a connected style element before its backing sheet is ready, so wait for
  // the sheet before dynamically importing FullCalendar.
  for (let attempt = 0; attempt < MAX_STYLE_SHEET_WAIT_FRAMES; attempt += 1) {
    if (style.sheet) return;
    await nextFrame();
  }
  if (!style.sheet) {
    throw new Error("FullCalendar stylesheet is not available yet.");
  }
}
