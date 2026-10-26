import { useCallback, useEffect, useRef, useState } from "react";
import { ChevronRight, Download, Loader2, Check, X } from "lucide-react";
import { useUi, useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { componentText } from "@/lib/ui-text-components";
import { errorText, type ServerError } from "@/lib/ui-text-errors";
import { programName } from "@/lib/programs";
import { SELF_POLL, hubChanged, setTelemetry, telemetryPreview, useHubState } from "@/lib/studioHub";

// Первый запуск: докачка того, чего нет на диске.
//
// В установщик веса не кладутся — он распух бы на десятки гигабайт. Приложение ставится
// пустым и добирает недостающее само, показывая, что качается и сколько осталось. Варианты
// одной стадии (распознавание речи) — одна группа: выбирается один, скачанный засчитывается.

type Component = {
  id: string;
  title: string;
  note: string;
  /// Насколько нужен: required держит первый запуск, recommended — заметно улучшает,
  /// optional — дело вкуса.
  requirement: "required" | "recommended" | "optional";
  /// Группа взаимозаменяемых вариантов; пусто — компонент сам по себе.
  variantOf: string;
  present: boolean;
  haveBytes: number;
  selected: boolean;
  /// Чем сорвалась последняя закачка; null — не срывалась.
  error: string | null;
  /// Та же причина с кодом и полями — для перевода на язык окна.
  errorInfo: ServerError | null;
  /// Программа, у которой компонент взят в этом запуске вместо закачки.
  takenFrom: string | null;
  files: Array<{ bytes: number }>;
};

const gb = (bytes: number, unit: string) => `${(bytes / 1024 ** 3).toFixed(1)} ${unit}`;
const size = (component: Component) => component.files.reduce((sum, file) => sum + file.bytes, 0);
const missingBytes = (component: Component) => Math.max(0, size(component) - component.haveBytes);

export default function SetupPanel() {
  const language = useUiLanguage();
  const panel = panelText(language);
  const ui = useUi();
  const hub = useHubState(language);
  const [components, setComponents] = useState<Component[] | null>(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState("");
  /// Почему рассказчик идёт не на CUDA-сборке карты (glibc старше нужной); null — идёт на ней.
  const [narratorNote, setNarratorNote] = useState<ServerError | null>(null);
  const [progress, setProgress] = useState<{ title: string; have: number; total: number } | null>(null);
  /// Выбранный вариант в группе; null — ещё не трогали, берём выбранный в настройках.
  const [picked, setPicked] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<string | null>(null);
  const poll = useRef<number | null>(null);

  const title = useCallback(
    (component: Component) => componentText(language, component.id)?.title ?? component.title,
    [language],
  );
  const describe = useCallback(
    (component: Component) => componentText(language, component.id)?.note ?? component.note,
    [language],
  );

  const refresh = useCallback(async () => {
    try {
      const data = await (await fetch("/api/setup/status", SELF_POLL)).json();
      setComponents(data.components ?? []);
      setNarratorNote(data.narratorNote ?? null);
      return (data.components ?? []) as Component[];
    } catch {
      setNote(panel.componentsUnreadable);
      return [];
    }
  }, [panel]);

  useEffect(() => {
    void (async () => {
      const list = await refresh();
      // Пока не хватает обязательного — раздел открыт сам: играть всё равно нечем.
      if (list.some((component) => component.requirement === "required" && !component.present)) setOpen(true);
    })();
  }, [refresh]);

  useEffect(() => () => { if (poll.current) window.clearInterval(poll.current); }, []);

  // Галочка статистики на экране — выбор увиден: с этого момента хаб может отправлять отчёт.
  useEffect(() => {
    if (open && hub && !hub.telemetry.acknowledged && !hub.telemetry.disabledByEnv) {
      setTelemetry(hub.telemetry.enabled, true)
        .then(hubChanged)
        .catch((e: Error) => console.warn("[hub] the statistics choice was not saved:", e.message));
    }
  }, [open, hub]);

  const acknowledge = useCallback(() => {
    if (hub && !hub.telemetry.acknowledged) {
      setTelemetry(hub.telemetry.enabled, true)
        .then(hubChanged)
        .catch((e: Error) => console.warn("[hub] the statistics choice was not saved:", e.message));
    }
  }, [hub]);

  const download = useCallback(
    async (ids: string[]) => {
      if (!ids.length) return;
      acknowledge();
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
          // Пустой список — сбой чтения статуса, а не «всё скачано»: иначе разрыв связи
          // во время докачки выдал бы «готово» и спрятал бы прогресс.
          if (!watched.length) return;
          const failed = watched.find((component) => component.error && !component.present);
          if (failed) {
            if (poll.current) window.clearInterval(poll.current);
            setProgress(null);
            setBusy(false);
            setNote(failed.errorInfo ? errorText(failed.errorInfo) : panel.downloadFailed.replace("{error}", failed.error ?? ""));
            return;
          }
          const have = watched.reduce((sum, component) => sum + component.haveBytes, 0);
          const total = watched.reduce((sum, component) => sum + size(component), 0);
          const pending = watched.find((component) => !component.present);
          setProgress({ title: pending ? title(pending) : panel.ready, have, total });
          if (!pending) {
            if (poll.current) window.clearInterval(poll.current);
            setProgress(null);
            setBusy(false);
          }
        }, 1500);
      } catch (error) {
        setNote(panel.downloadFailed.replace("{error}", error instanceof Error ? error.message : String(error)));
        setBusy(false);
      }
    },
    [refresh, acknowledge, panel, title],
  );

  /// Выбрать уже скачанный вариант: только настройка, без закачки.
  const choose = useCallback(async (component: Component) => {
    const body =
      component.id === "asr"
        ? { asrEngine: "parakeet", asrModel: "int8" }
        : component.id === "asr-ultra"
          ? { asrEngine: "parakeet", asrModel: "ultra" }
          : { asrEngine: "crisperwhisper" };
    try {
      const response = await fetch("/api/runtime", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
      if (!response.ok) throw new Error(await response.text());
      await refresh();
    } catch (error) {
      setNote(panel.notSaved.replace("{error}", error instanceof Error ? error.message : String(error)));
    }
  }, [refresh, panel]);

  if (!components) return null;

  const groups = [...new Set(components.filter((c) => c.variantOf).map((c) => c.variantOf))];
  const chosenIn = (group: string): Component | undefined => {
    const members = components.filter((c) => c.variantOf === group);
    const id = picked[group];
    return (
      members.find((c) => c.id === id) ??
      members.find((c) => c.selected) ??
      members.find((c) => c.requirement !== "optional") ??
      members[0]
    );
  };
  const covered = (group: string) => components.some((c) => c.variantOf === group && c.present);
  const singles = components.filter((c) => !c.variantOf);
  // Кнопка, как в Dub, берёт всё недостающее, кроме необязательного: из группы вариантов — выбранный,
  // если стадию ещё нечем считать.
  const missingSingles = singles.filter((c) => c.requirement !== "optional" && !c.present);
  const uncoveredGroups = groups
    .filter((group) => !covered(group))
    .map(chosenIn)
    .filter((c): c is Component => Boolean(c) && c!.requirement !== "optional");
  const pickedDownloads = groups
    .map(chosenIn)
    .filter((c): c is Component => Boolean(c) && !c!.present && picked[c!.variantOf] === c!.id);
  const toDownload = [...missingSingles, ...uncoveredGroups, ...pickedDownloads].filter(
    (c, index, list) => list.findIndex((other) => other.id === c.id) === index,
  );
  // «Не хватает главного» и «Ещё нужно» — только обязательное: без рекомендованного игра идёт.
  const owed = [...missingSingles, ...uncoveredGroups].filter((c) => c.requirement === "required");
  const missing = components.filter((component) => !component.present && (!component.variantOf || !covered(component.variantOf)));
  const totalMissing = missing.reduce((sum, component) => sum + missingBytes(component), 0);
  const telemetryOn = hub?.telemetry.enabled ?? true;

  const row = (component: Component) => (
    <li key={component.id} className="flex items-start gap-2 rounded border border-stone-800 bg-stone-950 px-3 py-2">
      <span className="mt-0.5 shrink-0">
        {component.present ? (
          <Check className="size-4 text-amber-200" aria-hidden="true" />
        ) : (
          <span className="block size-4 rounded-full border border-stone-700" />
        )}
      </span>
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline justify-between gap-2">
          <span className="truncate text-sm text-stone-200">{title(component)}</span>
          <span className="shrink-0 text-[11px] tabular-nums text-stone-500">{gb(size(component), panel.gbUnit)}</span>
        </span>
        <span className="block text-[11px] leading-relaxed text-stone-600">
          {describe(component)}
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
  );

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
            {owed.length === 0
              ? missing.length === 0
                ? ui.everythingInPlace
                : panel.optionalAvailableCount.replace("{n}", String(missing.length))
              : panel.missingRequiredCount.replace("{n}", String(owed.length))}
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
          {narratorNote && <p className="px-1 text-[11px] text-stone-500">{errorText(narratorNote)}</p>}
          {toDownload.length > 0 && (
            <button
              type="button"
              disabled={busy}
              onClick={() => void download(toDownload.map((c) => c.id))}
              className="flex w-full items-center justify-center gap-2 rounded bg-amber-200 px-3 py-2 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500"
            >
              <Download className="size-4" aria-hidden="true" />
              {panel.downloadSelected.replace("{size}", gb(toDownload.reduce((s, c) => s + missingBytes(c), 0), panel.gbUnit))}
            </button>
          )}
          {owed.length > 0 && !busy && (
            <p className="text-xs leading-relaxed text-stone-300">
              {panel.stillNeeded.replace("{list}", owed.map(title).join(", "))}
            </p>
          )}

          {progress && (
            <div className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
              <div className="flex items-baseline justify-between text-xs">
                <span className="truncate text-stone-300">{progress.title}</span>
                <span className="tabular-nums text-stone-500">
                  {gb(progress.have, panel.gbUnit)} / {gb(progress.total, panel.gbUnit)}
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

          <ul className="space-y-1.5">{singles.map(row)}</ul>

          {groups.map((group) => {
            const members = components.filter((c) => c.variantOf === group);
            const chosen = chosenIn(group);
            return (
              <div key={group} className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
                <div className="mb-1.5 flex items-baseline justify-between gap-2">
                  <span className="text-sm text-stone-200">{group === "asr" ? panel.variantGroupAsr : group}</span>
                  <span className="text-[10px] uppercase tracking-wide text-amber-200/80">{panel.pickOneVariant}</span>
                </div>
                <div className="space-y-0.5">
                  {members.map((component) => (
                    <label
                      key={component.id}
                      className={`flex cursor-pointer items-start gap-2 rounded px-1.5 py-1 hover:bg-stone-900 ${chosen?.id === component.id ? "bg-stone-900" : ""}`}
                    >
                      <input
                        type="radio"
                        name={`variant-${group}`}
                        checked={chosen?.id === component.id}
                        disabled={busy}
                        onChange={() => {
                          setPicked((current) => ({ ...current, [group]: component.id }));
                          if (component.present) void choose(component);
                        }}
                        className="mt-1 size-3.5 shrink-0 accent-amber-200"
                      />
                      <span className="min-w-0 flex-1">
                        <span className="flex items-baseline justify-between gap-2">
                          <span className="truncate text-sm text-stone-200">
                            {title(component)}
                            {component.requirement !== "optional" && !covered(group) && (
                              <span className="ml-1.5 text-[10px] uppercase text-amber-200/70">{panel.defaultWord}</span>
                            )}
                            {component.present && (
                              <span className="ml-1.5 text-[10px] text-amber-200">
                                {component.takenFrom ? panel.takenFrom.replace("{app}", programName(component.takenFrom)) : panel.installedWord}
                              </span>
                            )}
                          </span>
                          <span className="shrink-0 text-[11px] tabular-nums text-stone-500">{gb(size(component), panel.gbUnit)}</span>
                        </span>
                        <span className="block text-[11px] leading-relaxed text-stone-600">{describe(component)}</span>
                      </span>
                    </label>
                  ))}
                </div>
              </div>
            );
          })}

          {missing.length > 0 && (
            <p className="text-[11px] leading-relaxed text-stone-600">
              {panel.resumableNote.replace("{size}", gb(totalMissing, panel.gbUnit))}
            </p>
          )}

          {hub && !hub.telemetry.disabledByEnv && (
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-stone-400">
              <label className="inline-flex cursor-pointer items-center gap-2">
                <input
                  type="checkbox"
                  className="size-4 accent-amber-200"
                  checked={telemetryOn}
                  onChange={(event) => {
                    setTelemetry(event.target.checked, true)
                      .then(hubChanged)
                      .catch((e: Error) => console.warn("[hub] the statistics choice was not saved:", e.message));
                  }}
                />
                {panel.hubCheckbox}
              </label>
              <button
                type="button"
                onClick={() => {
                  telemetryPreview()
                    .then((data) => setPreview(data.enabled ? JSON.stringify(data.report, null, 2) : ""))
                    .catch((e: Error) => setNote(e.message));
                }}
                className="underline underline-offset-2 hover:text-stone-200"
              >{panel.hubWhat}</button>
            </div>
          )}
          {preview !== null && (
            <div className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
              <div className="mb-1 flex items-center justify-between gap-2">
                <span className="text-sm text-stone-200">{panel.hubPreviewTitle}</span>
                <button type="button" onClick={() => setPreview(null)} aria-label={panel.hubClose} className="text-stone-500 hover:text-stone-200">
                  <X className="size-4" />
                </button>
              </div>
              <p className="text-[11px] leading-relaxed text-stone-500">{panel.hubPreviewNote}</p>
              {preview ? (
                <pre className="mt-2 max-h-60 overflow-auto whitespace-pre-wrap text-[10px] leading-snug text-stone-400">{preview}</pre>
              ) : (
                <p className="mt-2 text-xs text-stone-400">{panel.hubPreviewOff}</p>
              )}
            </div>
          )}
          {note && <p className="text-xs leading-relaxed text-stone-500">{note}</p>}
        </div>
      )}
    </section>
  );
}
