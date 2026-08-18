import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronRight, Download, Loader2, Check } from "lucide-react";
import { useUi } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { useUiLanguage } from "@/lib/ui-text-context";

// Первый запуск: докачка того, чего нет на диске.
//
// В установщик веса не кладутся — он распух бы на десятки гигабайт. Приложение ставится
// пустым и добирает недостающее само, показывая, что качается и сколько осталось.

type Component = {
  id: string;
  title: string;
  note: string;
  /// Насколько нужен: required держит первый запуск, recommended — заметно улучшает,
  /// optional — дело вкуса.
  requirement: "required" | "recommended" | "optional";
  present: boolean;
  haveBytes: number;
  files: Array<{ bytes: number }>;
};

const gb = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(1)} ГБ`;
const size = (component: Component) => component.files.reduce((sum, file) => sum + file.bytes, 0);

export default function SetupPanel() {
  const panel = panelText(useUiLanguage());
  const ui = useUi();
  const [components, setComponents] = useState<Component[] | null>(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  const [progress, setProgress] = useState<{ title: string; have: number; total: number } | null>(null);
  const poll = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await (await fetch("/api/setup/status")).json();
      setComponents(data.components ?? []);
      return (data.components ?? []) as Component[];
    } catch {
      setNote(panel.componentsUnreadable);
      return [];
    }
  }, []);

  useEffect(() => {
    void (async () => {
      const list = await refresh();
      // Пока не хватает обязательного — раздел открыт сам: играть всё равно нечем.
      if (list.some((component) => component.requirement === "required" && !component.present)) setOpen(true);
    })();
  }, [refresh]);

  useEffect(() => () => { if (poll.current) window.clearInterval(poll.current); }, []);

  const download = useCallback(
    async (ids: string[]) => {
      setBusy(true);
      setNote("");
      try {
        const response = await fetch("/api/setup/download", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ ids }),
        });
        if (!response.ok) throw new Error(await response.text());
        // Прогресс считаем по байтам на диске: статус учитывает и незавершённые части,
        // поэтому шкала не обнуляется после перезапуска игры.
        poll.current = window.setInterval(async () => {
          const list = await refresh();
          const watched = list.filter((component) => ids.includes(component.id));
          const have = watched.reduce((sum, component) => sum + component.haveBytes, 0);
          const total = watched.reduce((sum, component) => sum + size(component), 0);
          const pending = watched.find((component) => !component.present);
          setProgress({ title: pending ? pending.title : panel.ready, have, total });
          if (!pending) {
            if (poll.current) window.clearInterval(poll.current);
            setProgress(null);
            setBusy(false);
          }
        }, 1500);
      } catch (error) {
        setNote(`Не скачалось: ${error instanceof Error ? error.message : String(error)}`);
        setBusy(false);
      }
    },
    [refresh],
  );

  if (!components) return null;

  const missing = components.filter((component) => !component.present);
  const missingRequired = missing.filter((component) => component.requirement === "required");
  const totalMissing = missing.reduce((sum, component) => sum + size(component) - component.haveBytes, 0);

  return (
    <section className="space-y-2.5">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full min-w-0 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70"
      >
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          <img src="/sidebar-icons/local-data.png" alt="" className="size-full object-cover" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-stone-300">{ui.whatToDownload}</span>
          <span className="block truncate text-xs text-stone-500">
            {missing.length === 0
              ? ui.everythingInPlace
              : missingRequired.length > 0
                ? `Не хватает главного: ${missingRequired.length}`
                : `Можно доставить: ${missing.length}`}
          </span>
        </span>
        {busy ? (
          <Loader2 className="ml-auto size-4 shrink-0 animate-spin text-amber-200" aria-hidden="true" />
        ) : (
          <ChevronRight
            className={`ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80 ${open ? "rotate-90 text-amber-200" : ""}`}
            aria-hidden="true"
          />
        )}
      </button>

      {open && (
        <div className="space-y-3">
          {missing.length > 0 && (
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                void download(
                  missing.filter((c) => c.requirement === "required").map((c) => c.id),
                )
              }
              className="flex w-full items-center justify-center gap-2 rounded bg-amber-200 px-3 py-2 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500"
            >
              <Download className="size-4" aria-hidden="true" />
              {missingRequired.length > 0
                ? `Скачать нужное — ${gb(missingRequired.reduce((s, c) => s + size(c) - c.haveBytes, 0))}`
                : `Всё главное на месте`}
            </button>
          )}

          {progress && (
            <div className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
              <div className="flex items-baseline justify-between text-xs">
                <span className="truncate text-stone-300">{progress.title}</span>
                <span className="tabular-nums text-stone-500">
                  {gb(progress.have)} / {gb(progress.total)}
                </span>
              </div>
              <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-stone-800">
                <div
                  className="h-full rounded-full bg-amber-200 transition-[width] duration-500"
                  style={{ width: `${progress.total ? Math.min(100, (progress.have / progress.total) * 100) : 0}%` }}
                />
              </div>
            </div>
          )}

          <ul className="space-y-1.5">
            {components.map((component) => (
              <li
                key={component.id}
                className="flex items-start gap-2 rounded border border-stone-800 bg-stone-950 px-3 py-2"
              >
                <span className="mt-0.5 shrink-0">
                  {component.present ? (
                    <Check className="size-4 text-amber-200" aria-hidden="true" />
                  ) : (
                    <span className="block size-4 rounded-full border border-stone-700" />
                  )}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="flex items-baseline justify-between gap-2">
                    <span className="truncate text-sm text-stone-200">{component.title}</span>
                    <span className="shrink-0 text-[11px] tabular-nums text-stone-500">{gb(size(component))}</span>
                  </span>
                  <span className="block text-[11px] leading-relaxed text-stone-600">
                    {component.note}
                    {component.requirement === "required"
                      ? panel.requiredNote
                      : component.requirement === "recommended"
                        ? panel.recommendedNote
                        : panel.optionalNote}
                  </span>
                </span>
                {!component.present && !busy && (
                  <button
                    type="button"
                    onClick={() => void download([component.id])}
                    className="shrink-0 self-center rounded border border-stone-700 px-2 py-1 text-xs text-stone-300 hover:border-amber-700/60 hover:text-amber-200"
                  >{panel.download}</button>
                )}
              </li>
            ))}
          </ul>

          {missing.length > 0 && (
            <p className="text-[11px] leading-relaxed text-stone-600">
              Всего не хватает {gb(totalMissing)}. Качается с докачкой: прерванная загрузка
              продолжится с того же места, а не начнётся заново.
            </p>
          )}
          {note && <p className="text-xs leading-relaxed text-stone-500">{note}</p>}
        </div>
      )}
    </section>
  );
}
