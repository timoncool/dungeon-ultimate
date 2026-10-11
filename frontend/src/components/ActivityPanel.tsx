import { useCallback, useEffect, useState } from "react";
import { Bot, ChevronRight } from "lucide-react";
import { useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { CHANGED } from "@/lib/mcpBridge";

// Журнал «Активность» — что агент сделал в игре через MCP, как журнал студий.

type Line = { at: string; tool: string; args: Record<string, unknown>; ok: boolean; summary: string };
type Status = { agent_connected: boolean; window_open: boolean; requests_waiting: number };

export default function ActivityPanel() {
  const panel = panelText(useUiLanguage());
  const [open, setOpen] = useState(false);
  const [lines, setLines] = useState<Line[]>([]);
  const [status, setStatus] = useState<Status | null>(null);

  const read = useCallback(async () => {
    try {
      const [activity, connection] = await Promise.all([
        fetch("/api/activity", { cache: "no-store" }).then((r) => r.json()),
        fetch("/api/mcp/status", { cache: "no-store" }).then((r) => r.json()),
      ]);
      setLines((activity.lines ?? []) as Line[]);
      setStatus(connection as Status);
    } catch (e) {
      console.warn("[activity]", e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void read();
    const listener = () => void read();
    window.addEventListener(CHANGED, listener);
    const timer = window.setInterval(read, 15_000);
    return () => {
      window.removeEventListener(CHANGED, listener);
      window.clearInterval(timer);
    };
  }, [read]);

  return (
    <section className="space-y-2.5">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full min-w-0 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70"
      >
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 text-amber-200 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          <Bot className="size-6" aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-stone-300">{panel.activity}</span>
          <span className="block truncate text-xs text-stone-500">
            {status?.agent_connected ? panel.agentConnected : panel.agentNotConnected}
            {status && status.requests_waiting > 0 ? ` · ${panel.agentRequestsWaiting.replace("{n}", String(status.requests_waiting))}` : ""}
          </span>
        </span>
        <ChevronRight
          className={`ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80 ${open ? "rotate-90 text-amber-200" : ""}`}
          aria-hidden="true"
        />
      </button>
      {open && (
        <div className="space-y-1.5">
          <p className="text-[11px] leading-relaxed text-stone-600">{panel.mcpHint.replace("{url}", `${window.location.origin}/mcp`)}</p>
          {lines.length === 0 && <p className="text-xs text-stone-500">{panel.activityEmpty}</p>}
          <ul className="space-y-1">
            {[...lines].reverse().map((line, index) => (
              <li key={`${line.at}-${index}`} className="rounded border border-stone-800 bg-stone-950 px-2.5 py-1.5">
                <div className="flex items-baseline justify-between gap-2">
                  <span className={`text-xs ${line.ok ? "text-stone-200" : "text-red-300"}`}>{line.tool}</span>
                  <span className="shrink-0 text-[10px] tabular-nums text-stone-600">{new Date(line.at).toLocaleTimeString()}</span>
                </div>
                <p className="mt-0.5 line-clamp-2 text-[11px] leading-snug text-stone-500">{line.summary}</p>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
