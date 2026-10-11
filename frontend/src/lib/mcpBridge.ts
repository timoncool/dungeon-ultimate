/**
 * Сторона окна в мосте MCP — как в YuE2 Studio: агент, подключённый к MCP игры, видит это окно
 * и работает с ним как человек — скриншот, что можно нажать, клики и ввод, разделы, звук.
 *
 * Сервер шлёт сюда каждую команду и ждёт ответа, который окно отправляет обратно. Команды,
 * которые принадлежат экрану (разделы, звук), отвечает App через `useBridgeCommand`.
 */
import { useEffect, useRef } from "react";
import { captureWindow } from "./windowScreenshot";

type Handler = (args: Record<string, unknown>) => unknown | Promise<unknown>;

const handlers = new Map<string, Handler>();

/** Отвечать на команду, пока компонент смонтирован. */
export function useBridgeCommand(command: string, handler: Handler): void {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    const run: Handler = (args) => latest.current(args);
    handlers.set(command, run);
    return () => {
      if (handlers.get(command) === run) handlers.delete(command);
    };
  }, [command]);
}

// ---------------------------------------------------------------- что на экране

const INTERACTIVE = 'button, a[href], input, textarea, select, [role="button"], [role="tab"], [role="menuitem"], [role="checkbox"], [role="switch"], [role="slider"], [contenteditable="true"]';

function visible(element: Element): boolean {
  const box = element.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return false;
  const style = getComputedStyle(element);
  return style.visibility !== "hidden" && style.display !== "none";
}

/// Имя иконки lucide, которую кнопка показывает вместо слов.
function iconName(element: Element): string {
  const classes = element.querySelector("svg")?.getAttribute("class") ?? "";
  const icon = classes.split(/\s+/).find((name) => name.startsWith("lucide-"));
  return icon ? `${icon.slice("lucide-".length).replace(/-/g, " ")} icon` : "";
}

/// Текст рядом с полем: подпись, заголовок, строка перед ним.
function nearbyText(element: Element): string {
  let node: Element | null = element;
  for (let depth = 0; depth < 2 && node; depth += 1) {
    for (let sibling = node.previousElementSibling; sibling; sibling = sibling.previousElementSibling) {
      if (sibling.matches(INTERACTIVE) || sibling.querySelector(INTERACTIVE) || sibling.matches("h1, h2") || sibling.querySelector("h1, h2")) continue;
      const text = (sibling.textContent || "").replace(/\s+/g, " ").trim();
      if (text && text.length <= 80) return text;
    }
    node = node.parentElement;
  }
  return "";
}

function label(element: Element): string {
  const own = element.getAttribute("aria-label") || element.getAttribute("title") || "";
  if (own) return own.slice(0, 80);
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
    const tied = element.labels?.[0]?.textContent?.replace(/\s+/g, " ").trim();
    return (tied || nearbyText(element) || element.getAttribute("placeholder") || "").slice(0, 80);
  }
  const text = (element.textContent || "").replace(/\s+/g, " ").trim();
  return (text || iconName(element) || nearbyText(element)).slice(0, 80);
}

let nextRef = 1;

function refOf(element: Element): string {
  const existing = element.getAttribute("data-mcp-ref");
  if (existing) return existing;
  const ref = `e${nextRef++}`;
  element.setAttribute("data-mcp-ref", ref);
  return ref;
}

/// Диалог сверху, если открыт: до его кнопок игрок и дотягивается.
function openDialog(): Element | null {
  const dialogs = Array.from(document.querySelectorAll('[role="dialog"], [role="alertdialog"], [aria-modal="true"]')).filter(visible);
  return dialogs.length ? dialogs[dialogs.length - 1] : null;
}

/// Каждый видимый элемент управления одной строкой, с ref для других команд.
function readPage(): string {
  const lines: string[] = [];
  const title = document.querySelector("h1, h2")?.textContent?.trim();
  if (title) lines.push(`Page: ${title}`);
  const dialog = openDialog();
  if (dialog) lines.push("A dialog is open; its controls are listed first, then the page behind it.");
  const scope = dialog ? [dialog, document.body] : [document.body];
  const seen = new Set<Element>();
  for (const root of scope) {
    for (const element of Array.from(root.querySelectorAll(INTERACTIVE))) {
      if (seen.has(element) || !visible(element)) continue;
      seen.add(element);
      const tag = element.tagName.toLowerCase();
      const kind = element.getAttribute("role") || (tag === "input" ? `input ${(element as HTMLInputElement).type}` : tag);
      const parts = [refOf(element), kind, JSON.stringify(label(element))];
      if (element instanceof HTMLInputElement && (element.type === "checkbox" || element.type === "radio")) {
        parts.push(element.checked ? "checked" : "unchecked");
      } else if (element instanceof HTMLInputElement && element.type === "range") {
        parts.push(`value=${element.value}`, `range=${element.min || 0}..${element.max || 100}`, `step=${element.step || 1}`);
      } else if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
        if (element.value) parts.push(`value=${JSON.stringify(element.value.slice(0, 160))}`);
        else parts.push("empty", ...(element.placeholder ? [`placeholder=${JSON.stringify(element.placeholder.slice(0, 80))}`] : []));
      }
      if (element instanceof HTMLSelectElement) {
        parts.push(`value=${JSON.stringify(element.value)}`, `options=${JSON.stringify(Array.from(element.options).map((option) => option.value))}`);
      }
      const pressed = element.getAttribute("aria-checked") ?? element.getAttribute("aria-pressed") ?? element.getAttribute("aria-selected") ?? element.getAttribute("aria-expanded");
      if (pressed) parts.push(pressed === "true" ? "on" : "off");
      if ((element as HTMLButtonElement).disabled) parts.push("disabled");
      lines.push(parts.join(" "));
    }
  }
  return lines.join("\n");
}

function find(args: Record<string, unknown>): HTMLElement {
  const ref = typeof args.ref === "string" ? args.ref : "";
  const text = typeof args.text === "string" ? args.text.trim().toLowerCase() : "";
  let element: Element | null = null;
  if (ref) element = document.querySelector(`[data-mcp-ref="${CSS.escape(ref)}"]`);
  if (!element && text) {
    element = Array.from(document.querySelectorAll(INTERACTIVE)).find((candidate) => visible(candidate) && label(candidate).toLowerCase() === text)
      ?? Array.from(document.querySelectorAll(INTERACTIVE)).find((candidate) => visible(candidate) && label(candidate).toLowerCase().includes(text))
      ?? null;
  }
  if (!element) throw new Error(ref ? `No element ${ref} on the page now; call ui_read_page again, refs change when the page does.` : `No control labelled "${args.text}" is visible; call ui_read_page to see what is.`);
  return element as HTMLElement;
}

/** Ставит значение так же, как ввод с клавиатуры, чтобы React его увидел. */
function setValue(element: HTMLElement, value: string): void {
  if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) {
    const prototype = element instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype;
    Object.getOwnPropertyDescriptor(prototype, "value")?.set?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  } else if (element.isContentEditable) {
    element.textContent = value;
    element.dispatchEvent(new InputEvent("input", { bubbles: true }));
  } else {
    throw new Error("That element does not take text.");
  }
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 250));

// ---------------------------------------------------------------- что окно записало в консоль

const logged: Array<{ level: string; text: string; at: string }> = [];

function remember(level: string, parts: unknown[]): void {
  const text = parts.map((part) => (part instanceof Error ? `${part.name}: ${part.message}` : typeof part === "string" ? part : JSON.stringify(part))).join(" ");
  logged.push({ level, text: text.slice(0, 2000), at: new Date().toISOString() });
  if (logged.length > 100) logged.shift();
}

function watchConsole(): void {
  for (const level of ["error", "warn"] as const) {
    const original = console[level].bind(console);
    console[level] = (...parts: unknown[]) => {
      remember(level, parts);
      original(...parts);
    };
  }
  window.addEventListener("error", (event) => remember("error", [event.message, `${event.filename}:${event.lineno}`]));
  window.addEventListener("unhandledrejection", (event) => remember("error", ["unhandled rejection", event.reason]));
}

const builtIn: Record<string, Handler> = {
  async screenshot(args) {
    if (document.visibilityState === "hidden") {
      throw new Error("The game's window is hidden - minimised or covered by other windows - so it draws nothing to copy. Ask the player to bring it to the front, or use ui_read_page, which works either way.");
    }
    const data = await captureWindow(Number(args.max_width ?? 1600));
    return { image: data.replace(/^data:image\/png;base64,/, ""), text: `${window.innerWidth}x${window.innerHeight} window` };
  },
  read_page: () => ({ text: readPage() }),
  console: () => ({ text: logged.length ? logged.map((entry) => `${entry.at} ${entry.level}: ${entry.text}`).join("\n") : "Nothing logged." }),
  async click(args) {
    const element = find(args);
    element.scrollIntoView({ block: "center" });
    element.click();
    await settle();
    return { text: `Clicked ${label(element) || element.tagName.toLowerCase()}.` };
  },
  async type(args) {
    const element = find(args);
    element.focus();
    setValue(element, String(args.value ?? ""));
    if (args.submit) element.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await settle();
    return { text: "Typed." };
  },
  async select(args) {
    const element = find(args);
    if (!(element instanceof HTMLSelectElement)) throw new Error("That element is not a list; use ui_click on its options.");
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value")?.set?.call(element, String(args.value ?? ""));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    await settle();
    return { text: `Selected ${element.value}.` };
  },
  async press_key(args) {
    const target = (document.activeElement as HTMLElement | null) ?? document.body;
    const combo = String(args.key ?? "");
    const parts = combo.split("+").map((part) => part.trim()).filter(Boolean);
    const key = parts.length > 1 ? parts[parts.length - 1] : combo;
    const held = parts.slice(0, -1).map((part) => part.toLowerCase());
    const init = {
      key: key.length === 1 ? key.toLowerCase() : key,
      bubbles: true,
      cancelable: true,
      ctrlKey: held.includes("ctrl") || held.includes("control"),
      shiftKey: held.includes("shift"),
      altKey: held.includes("alt"),
      metaKey: held.includes("meta") || held.includes("cmd") || held.includes("win"),
    };
    target.dispatchEvent(new KeyboardEvent("keydown", init));
    target.dispatchEvent(new KeyboardEvent("keyup", init));
    await settle();
    return { text: `Pressed ${combo}.` };
  },
  async scroll(args) {
    const amount = Number(args.amount ?? 600) * (args.direction === "up" ? -1 : 1);
    const element = args.ref || args.text ? find(args) : null;
    if (element) element.scrollIntoView({ block: "center" });
    else (document.querySelector("main section") ?? document.scrollingElement ?? document.body).scrollBy({ top: amount });
    await settle();
    return { text: "Scrolled." };
  },
};

// ---------------------------------------------------------------- связь

/** Что агент поменял за спиной окна: окно перечитывает истории и настройки. */
export const CHANGED = "du:changed";
/** Клип, который окно должно проиграть: озвучка хода агента или ui_play. */
export const PLAY = "du:play";

let started = false;

export function startBridge(): void {
  if (started || typeof window === "undefined" || typeof EventSource === "undefined") return;
  started = true;
  watchConsole();
  let reconnecting = false;
  let current: number | null = null;
  const reportFocus = () => {
    if (current === null || document.visibilityState !== "visible" || !document.hasFocus()) return;
    void fetch("/mcp/window/focus", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ window: current }) })
      .catch((problem) => console.error("[ERROR] the game did not learn which window is in front:", problem));
  };
  window.addEventListener("focus", reportFocus);
  document.addEventListener("visibilitychange", reportFocus);
  const connect = () => {
    const events = new EventSource("/mcp/window");
    let ours: number | null = null;
    events.onmessage = async (message) => {
      const data = JSON.parse(message.data) as { window?: number; id?: string; command?: string; args?: Record<string, unknown>; changed?: string; play?: string; chat_id?: string };
      if (data.changed) {
        window.dispatchEvent(new CustomEvent(CHANGED, { detail: data.changed }));
        return;
      }
      if (data.play) {
        window.dispatchEvent(new CustomEvent(PLAY, { detail: { url: data.play, chatId: data.chat_id } }));
        return;
      }
      if (data.id === undefined) {
        ours = data.window ?? null;
        current = ours;
        reportFocus();
        if (reconnecting) window.dispatchEvent(new CustomEvent(CHANGED, { detail: "everything" }));
        return;
      }
      if (data.window !== ours) return;
      const command = data as { id: string; command: string; args: Record<string, unknown> };
      const handler = handlers.get(command.command) ?? builtIn[command.command];
      let body: { id: string; result?: unknown; error?: string };
      if (!handler) {
        body = { id: command.id, error: `The window cannot do "${command.command}" on this screen.` };
      } else {
        try {
          body = { id: command.id, result: (await handler(command.args ?? {})) ?? null };
        } catch (problem) {
          body = { id: command.id, error: problem instanceof Error ? problem.message : String(problem) };
        }
      }
      await fetch("/mcp/window/result", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) })
        .catch((problem) => console.error("[ERROR] the MCP answer did not reach the game:", problem));
    };
    events.onerror = () => {
      events.close();
      reconnecting = true;
      setTimeout(connect, 2000);
    };
  };
  connect();
}
