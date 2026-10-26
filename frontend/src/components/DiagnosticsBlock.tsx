import { useCallback, useEffect, useState } from "react";
import { Check, Copy, FolderOpen, Loader2, Package, RefreshCw } from "lucide-react";
import { SELF_POLL } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { settingsText } from "@/lib/ui-text-settings";

type Diagnostics = { text: string };

/** The computer as the game sees it (cards, builds, components, missing libraries, the latest log), to copy into a
 * problem report or to pack with the activity journal and the log; keys never get in (Speech Studio DiagnosticsPanel). */
export default function DiagnosticsBlock() {
  const text = settingsText(useUiLanguage());
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [packing, setPacking] = useState(false);
  const [pack, setPack] = useState<string | null>(null);
  const call = useCallback(async <T,>(path: string, body?: unknown): Promise<T> => {
    const response = await fetch(path, body === undefined ? SELF_POLL : { method: "POST", headers: { "Content-Type": "application/json", "x-hub-skip": "1" }, body: JSON.stringify(body) });
    const answer = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(answer.detail ?? answer.error ?? `HTTP ${response.status}`);
    return answer as T;
  }, []);
  const load = useCallback(() => {
    call<Diagnostics>("/api/diagnostics").then((d) => { setDiag(d); setProblem(null); }, (e: Error) => setProblem(e.message));
  }, [call]);
  useEffect(load, [load]);
  const copy = () => {
    if (!diag) return;
    navigator.clipboard.writeText(diag.text).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 1500); }, (e: Error) => setProblem(e.message));
  };
  const makePack = async () => {
    setPacking(true);
    try { setPack((await call<{ path: string }>("/api/diagnostics/report", {})).path); setProblem(null); } catch (e) { setProblem((e as Error).message); } finally { setPacking(false); }
  };
  const btn = "inline-flex items-center gap-1.5 rounded border border-stone-700 bg-stone-900/40 px-2 py-1 text-[11px] text-stone-300 hover:border-amber-700/60 disabled:opacity-40";
  return (
    <div className="space-y-2 border-t border-stone-800 pt-3">
      <p className="text-[11px] uppercase tracking-wide text-stone-500">{text.diagTitle}</p>
      <p className="text-pretty text-[11px] leading-relaxed text-stone-500">{text.diagHint}</p>
      <div className="flex flex-wrap gap-1.5">
        <button type="button" onClick={copy} disabled={!diag} className={btn}>{copied ? <Check className="size-3.5 text-amber-200" aria-hidden="true" /> : <Copy className="size-3.5" aria-hidden="true" />}{copied ? text.diagCopied : text.diagCopy}</button>
        <button type="button" onClick={() => void makePack()} disabled={packing} className={btn}>{packing ? <Loader2 className="size-3.5 animate-spin" aria-hidden="true" /> : <Package className="size-3.5" aria-hidden="true" />}{text.diagPack}</button>
        <button type="button" onClick={() => call("/api/diagnostics/logs/reveal", {}).catch((e: Error) => setProblem(e.message))} className={btn}><FolderOpen className="size-3.5" aria-hidden="true" />{text.diagLogs}</button>
        <button type="button" onClick={load} className={btn}><RefreshCw className="size-3.5" aria-hidden="true" />{text.diagRefresh}</button>
      </div>
      {pack && (
        <p className="flex flex-wrap items-center gap-1.5 text-[11px]">
          <span className="text-stone-500">{text.diagPacked}</span>
          <span className="break-all font-mono text-stone-300">{pack}</span>
          <button type="button" onClick={() => call("/api/diagnostics/report/reveal", { path: pack }).catch((e: Error) => setProblem(e.message))} className="inline-flex items-center gap-1 text-stone-400 underline hover:text-amber-200"><FolderOpen className="size-3" aria-hidden="true" />{text.diagShow}</button>
        </p>
      )}
      {problem && <p role="alert" className="break-words font-mono text-[11px] text-red-300">{problem}</p>}
      {diag ? <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-words rounded border border-stone-800 bg-stone-950 p-2 font-mono text-[10px] leading-snug text-stone-400">{diag.text}</pre>
        : !problem && <Loader2 className="size-3.5 animate-spin text-stone-500" aria-hidden="true" />}
    </div>
  );
}
