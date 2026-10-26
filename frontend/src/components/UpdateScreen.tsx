import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { ArrowRight, Check, Download, Loader2, X } from "lucide-react";
import { SELF_POLL } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { componentText } from "@/lib/ui-text-components";
import { updateText } from "@/lib/ui-text-update";

type Row = { id: string; title: string; from: string | null; to: string; download: number; reason: string };
type Gone = { path: string; size: number; reason: string };
export type UpdatePlan = { version: string; updated: Row[]; added: Row[]; retired: Gone[]; download: number; freed: number; show: boolean };
type Status = { id: string; present: boolean; haveBytes: number; files?: { bytes: number }[]; error?: string | null };

const gb = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(1)} GB`;

/** The update screen's plan (Dub Studio UpdatePlanView): three groups, the totals and one button that downloads it all
 * and then removes what is no longer needed. `onClose` puts it away; `asked` — shown by the player, not at start. */
export default function UpdateScreen({ onClose, asked = false }: { onClose: () => void; asked?: boolean }) {
  const text = updateText(useUiLanguage());
  const language = useUiLanguage();
  const [plan, setPlan] = useState<UpdatePlan | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);
  const [progress, setProgress] = useState<{ have: number; total: number } | null>(null);
  const poll = useRef<number | null>(null);

  const load = useCallback(async () => {
    try {
      const response = await fetch("/api/setup/update", SELF_POLL);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      setPlan(await response.json());
    } catch (e) {
      setProblem((e as Error).message);
    }
  }, []);
  useEffect(() => { void load(); }, [load]);
  useEffect(() => () => { if (poll.current) window.clearInterval(poll.current); }, []);

  const post = async (path: string) => {
    const response = await fetch(path, { method: "POST", headers: { "Content-Type": "application/json", "x-hub-skip": "1" }, body: "{}" });
    const answer = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(answer.error ?? `HTTP ${response.status}`);
    return answer;
  };

  const apply = async () => {
    if (!plan) return;
    setProblem(null);
    setBusy(true);
    const ids = [...plan.updated, ...plan.added].map((row) => row.id);
    try {
      const answer = await post("/api/setup/update/apply");
      if (!answer.job) {
        setBusy(false);
        setDone(true);
        setPlan(answer.plan);
        return;
      }
      poll.current = window.setInterval(async () => {
        const list: Status[] = (await (await fetch("/api/setup/status", SELF_POLL)).json()).components ?? [];
        const watched = list.filter((c) => ids.includes(c.id));
        if (!watched.length) return;
        const failed = watched.find((c) => c.error && !c.present);
        const total = watched.reduce((s, c) => s + (c.files ?? []).reduce((a, f) => a + f.bytes, 0), 0);
        setProgress({ have: watched.reduce((s, c) => s + c.haveBytes, 0), total });
        if (failed || watched.every((c) => c.present)) {
          if (poll.current) window.clearInterval(poll.current);
          setBusy(false);
          setProgress(null);
          if (failed) setProblem(failed.error ?? "");
          else setDone(true);
          void load();
        }
      }, 1500);
    } catch (e) {
      setProblem((e as Error).message);
      setBusy(false);
    }
  };
  const later = () => {
    post("/api/setup/update/seen").then(onClose, (e: Error) => setProblem(e.message));
  };

  const reason = (code: string) => text.reasons[code] ?? code;
  const title = (id: string, fallback: string) => componentText(language, id)?.title ?? fallback;
  const empty = plan != null && plan.updated.length + plan.added.length + plan.retired.length === 0;
  const group = (label: string, rows: ReactNode[]) =>
    rows.length > 0 && (
      <div className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
        <div className="mb-1.5 text-[10px] uppercase tracking-wide text-stone-500">{label}</div>
        <div className="space-y-1.5">{rows}</div>
      </div>
    );
  const line = (key: string, name: string, version: ReactNode, size: string, why: string) => (
    <div key={key}>
      <div className="flex items-baseline gap-2">
        <span className="truncate text-sm text-stone-200">{name}</span>
        {version}
        <span className="ml-auto shrink-0 text-[11px] tabular-nums text-stone-500">{size}</span>
      </div>
      <div className="text-[11px] leading-relaxed text-stone-500">{why}</div>
    </div>
  );
  const version = (from: string | null, to: string) => (
    <span className="inline-flex min-w-0 items-center gap-1 truncate font-mono text-[10px] text-stone-500">
      {from && <>{from}<ArrowRight className="size-3 shrink-0" aria-hidden="true" /></>}{to}
    </span>
  );

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 px-4 py-8" role="dialog" aria-modal="true" aria-label={text.title}>
      <div className="max-h-full w-full max-w-xl overflow-y-auto rounded-lg border border-stone-700 bg-stone-900 p-5 shadow-2xl">
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="text-[10px] uppercase tracking-[0.2em] text-amber-200/80">{text.kicker} {plan?.version}</p>
            <h2 className="mt-1 text-lg font-semibold text-stone-100">{text.title}</h2>
          </div>
          {asked && !busy && (
            <button type="button" onClick={onClose} aria-label={text.later} className="text-stone-500 hover:text-stone-200"><X className="size-4" aria-hidden="true" /></button>
          )}
        </div>
        <p className="mb-3 mt-2 text-xs leading-relaxed text-stone-400">{text.subtitle}</p>
        <div className="space-y-2.5">
          {!plan && !problem && <Loader2 className="size-4 animate-spin text-stone-500" aria-hidden="true" />}
          {empty && <p className="text-xs text-stone-400">{text.nothing}</p>}
          {plan && group(text.updated, plan.updated.map((r) => line(r.id, title(r.id, r.title), version(r.from, r.to), gb(r.download), reason(r.reason))))}
          {plan && group(text.added, plan.added.map((r) => line(r.id, title(r.id, r.title), version(null, r.to), gb(r.download), reason(r.reason))))}
          {plan && group(text.retired, plan.retired.map((g) => line(g.path, g.path, null, g.size > 0 ? `−${gb(g.size)}` : "", reason(g.reason))))}
          {progress && (
            <div className="h-1.5 overflow-hidden rounded-full bg-stone-800">
              <div className="h-full rounded-full bg-amber-200 transition-[width] duration-500" style={{ width: `${progress.total ? Math.min(100, (progress.have / progress.total) * 100) : 0}%` }} />
            </div>
          )}
          {problem && <p role="alert" className="break-words font-mono text-[11px] text-red-300">{problem}</p>}
          {done && <p className="text-xs text-amber-200">{text.done}</p>}
          {plan && !empty && !done && (
            <div className="flex flex-wrap items-center gap-3 pt-1">
              <button type="button" onClick={() => void apply()} disabled={busy} className="inline-flex items-center gap-2 rounded bg-amber-200 px-3 py-2 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500">
                {busy ? <Loader2 className="size-4 animate-spin" aria-hidden="true" /> : plan.download > 0 ? <Download className="size-4" aria-hidden="true" /> : <Check className="size-4" aria-hidden="true" />}
                {text.apply}
              </button>
              <span className="text-[11px] tabular-nums text-stone-500">
                {[plan.download > 0 ? text.download.replace("{size}", gb(plan.download)) : "", plan.freed > 0 ? text.freed.replace("{size}", gb(plan.freed)) : ""].filter(Boolean).join(" · ")}
              </span>
              {!busy && <button type="button" onClick={asked ? onClose : later} className="ml-auto rounded border border-stone-700 px-3 py-2 text-sm text-stone-400 hover:text-stone-200">{text.later}</button>}
            </div>
          )}
          {(done || empty) && <button type="button" onClick={onClose} className="rounded border border-stone-700 px-3 py-2 text-sm text-stone-300 hover:text-amber-200">OK</button>}
        </div>
      </div>
    </div>
  );
}
