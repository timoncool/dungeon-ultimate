import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronRight, Loader2 } from "lucide-react";

// Настройки движков и облака.
//
// Всё это уже умел сервер, но дотянуться до него можно было только правкой файла: в игре
// не было ни выбора модели, ни переключателя «на карте / в облаке», ни размера контекста.
// Панель закрывает этот разрыв — четыре стадии хода переключаются по отдельности, поэтому
// играть можно и вовсе без видеокарты, и наполовину.

type Runtime = {
  narratorCtx: number;
  narratorGpuLayers: number;
  imageSteps: number;
  imageCfg: number;
  imageOffloadToCpu: boolean;
  imageMaxVram: string;
  ttsEnabled: boolean;
  ttsParallel: boolean;
  ttsReferenceSeconds: number;
  asrOnGpu: boolean;
  openrouterKey: string;
  openrouterNarratorOn: boolean;
  openrouterNarratorModel: string;
  openrouterImageOn: boolean;
  openrouterImageModel: string;
  openrouterTtsOn: boolean;
  openrouterTtsModel: string;
  openrouterTtsVoice: string;
  openrouterAsrOn: boolean;
  openrouterAsrModel: string;
};

type CloudVoice = { name: string; gender: string; age: string; ru: boolean };

/// Стадия хода: каждая живёт своей жизнью и переключается отдельно от остальных.
type Stage = {
  key: "narrator" | "image" | "tts" | "asr";
  title: string;
  kind: string;
  on: keyof Runtime;
  model: keyof Runtime;
  local: string;
};

const STAGES: Stage[] = [
  { key: "narrator", title: "Рассказчик", kind: "text", on: "openrouterNarratorOn", model: "openrouterNarratorModel", local: "Гемма на карте" },
  { key: "image", title: "Кадр", kind: "image", on: "openrouterImageOn", model: "openrouterImageModel", local: "Krea на карте" },
  { key: "tts", title: "Озвучка", kind: "tts", on: "openrouterTtsOn", model: "openrouterTtsModel", local: "Higgs на карте" },
  { key: "asr", title: "Речь в текст", kind: "asr", on: "openrouterAsrOn", model: "openrouterAsrModel", local: "Parakeet на карте" },
];

const box =
  "w-full rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-sm text-stone-200 outline-none focus:border-amber-700/60";

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="block text-xs font-medium uppercase text-stone-500">{label}</span>
      {children}
      {hint && <span className="block text-[11px] leading-relaxed text-stone-600">{hint}</span>}
    </label>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="flex items-center justify-between gap-3 rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-300">
      {label}
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="size-4 accent-amber-200"
      />
    </label>
  );
}

export default function EnginePanel() {
  const [runtime, setRuntime] = useState<Runtime | null>(null);
  const [keySet, setKeySet] = useState(false);
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [note, setNote] = useState("");
  const [models, setModels] = useState<Record<string, string[]>>({});
  const [voices, setVoices] = useState<CloudVoice[]>([]);
  const [keyDraft, setKeyDraft] = useState("");

  useEffect(() => {
    void (async () => {
      try {
        const data = await (await fetch("/api/runtime")).json();
        setRuntime(data.runtime);
        setKeySet(Boolean(data.openrouterKeySet));
      } catch {
        setNote("Настройки движков не читаются.");
      }
    })();
  }, []);

  // Списки моделей тянем только когда панель открыта: это сетевой запрос к провайдеру.
  useEffect(() => {
    if (!open || !keySet) return;
    for (const stage of STAGES) {
      if (models[stage.kind]) continue;
      void (async () => {
        try {
          const data = await (await fetch(`/api/cloud/models?kind=${stage.kind}`)).json();
          const list: string[] = (data.models ?? data).map((m: { id: string }) => m.id).sort();
          setModels((current) => ({ ...current, [stage.kind]: list }));
        } catch {
          setModels((current) => ({ ...current, [stage.kind]: [] }));
        }
      })();
    }
  }, [open, keySet, models]);

  const ttsModel = runtime?.openrouterTtsModel ?? "";
  useEffect(() => {
    if (!open || !keySet || !ttsModel) return;
    void (async () => {
      try {
        const data = await (await fetch(`/api/cloud/voices?model=${encodeURIComponent(ttsModel)}`)).json();
        setVoices(data.voices ?? []);
      } catch {
        setVoices([]);
      }
    })();
  }, [open, keySet, ttsModel]);

  const save = useCallback(async (patch: Partial<Runtime>) => {
    setSaving(true);
    setNote("");
    try {
      const response = await fetch("/api/runtime", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(patch),
      });
      if (!response.ok) throw new Error(await response.text());
      const data = await (await fetch("/api/runtime")).json();
      setRuntime(data.runtime);
      setKeySet(Boolean(data.openrouterKeySet));
      // Перезапуск не нужен: сервер перечитывает настройки на каждом запуске модели.
      setNote("Сохранено — действует со следующего хода.");
    } catch (error) {
      setNote(`Не сохранилось: ${error instanceof Error ? error.message : String(error)}`);
      // Переключатель мы двигаем сразу, не дожидаясь ответа. Раз ответа нет — возвращаем
      // то, что на сервере: иначе панель показывала бы облако там, где его не включили.
      try {
        const data = await (await fetch("/api/runtime")).json();
        setRuntime(data.runtime);
      } catch {
        // связи нет вовсе — оставляем как есть, сообщение об ошибке уже показано
      }
    } finally {
      setSaving(false);
    }
  }, []);

  const patch = useCallback(
    (key: keyof Runtime, value: string | number | boolean) => {
      setRuntime((current) => (current ? { ...current, [key]: value } : current));
      void save({ [key]: value } as Partial<Runtime>);
    },
    [save],
  );

  const cloudStages = useMemo(
    () => (runtime ? STAGES.filter((stage) => runtime[stage.on] === true).length : 0),
    [runtime],
  );

  if (!runtime) return null;

  return (
    // Шапка повторяет общий раздел настроек один в один: рисованная иконка в рамке, название,
    // стрелка справа. Общий компонент живёт в App.tsx, а импортировать его отсюда — это
    // круговая зависимость, поэтому разметка продублирована сознательно.
    <section className="space-y-2.5">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full min-w-0 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70"
      >
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          <img src="/sidebar-icons/engines.png" alt="" className="size-full object-cover" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-stone-300">Движки и облако</span>
          <span className="block truncate text-xs text-stone-500">
            {cloudStages === 0
              ? "Всё считает своя карта"
              : `В облаке ${cloudStages} из ${STAGES.length}`}
          </span>
        </span>
        {saving ? (
          <Loader2 className="ml-auto size-4 shrink-0 animate-spin text-stone-500" aria-hidden="true" />
        ) : (
          <ChevronRight
            className={`ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80 ${
              open ? "rotate-90 text-amber-200" : ""
            }`}
            aria-hidden="true"
          />
        )}
      </button>

      {open && (
        <div className="space-y-4">
          <div className="space-y-2">
            <p className="text-xs leading-relaxed text-stone-500">
              Ключ OpenRouter {keySet ? "задан" : "не задан"}. Без него облачные стадии не включаются
              и всё считает своя карта.
            </p>
            <div className="flex gap-2">
              <input
                type="password"
                value={keyDraft}
                placeholder={keySet ? "Заменить ключ" : "sk-or-…"}
                onChange={(event) => setKeyDraft(event.target.value)}
                className={box}
              />
              <button
                type="button"
                disabled={!keyDraft.trim() || saving}
                onClick={() => {
                  void save({ openrouterKey: keyDraft.trim() });
                  setKeyDraft("");
                }}
                className="shrink-0 rounded border border-stone-700 px-3 text-sm text-stone-200 hover:border-amber-700/60 disabled:cursor-not-allowed disabled:text-stone-600"
              >
                Сохранить
              </button>
            </div>
          </div>

          {STAGES.map((stage) => {
            const inCloud = runtime[stage.on] === true;
            const list = models[stage.kind] ?? [];
            const chosen = String(runtime[stage.model] ?? "");
            return (
              <div key={stage.key} className="space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-sm text-stone-200">{stage.title}</span>
                  {/* Переключатель не переносим: в две строки он читается как две кнопки. */}
                  <div className="flex shrink-0 overflow-hidden rounded border border-stone-800">
                    {[false, true].map((cloud) => (
                      <button
                        key={String(cloud)}
                        type="button"
                        disabled={cloud && !keySet}
                        onClick={() => patch(stage.on, cloud)}
                        className={`whitespace-nowrap px-2.5 py-1 text-xs ${
                          inCloud === cloud
                            ? "bg-amber-200 text-stone-950"
                            : "text-stone-400 hover:text-stone-200 disabled:text-stone-700"
                        }`}
                      >
                        {cloud ? "В облаке" : "На карте"}
                      </button>
                    ))}
                  </div>
                </div>
                {inCloud ? (
                  <>
                    <select
                      value={chosen}
                      onChange={(event) => patch(stage.model, event.target.value)}
                      className={box}
                    >
                      {chosen && !list.includes(chosen) && <option value={chosen}>{chosen}</option>}
                      {list.map((id) => (
                        <option key={id} value={id}>
                          {id}
                        </option>
                      ))}
                    </select>
                    {stage.key === "tts" && (
                      <Row label="Голос" hint="Отмечены голоса, которые тянут русскую речь.">
                        <select
                          value={runtime.openrouterTtsVoice}
                          onChange={(event) => patch("openrouterTtsVoice", event.target.value)}
                          className={box}
                        >
                          {voices.map((voice) => (
                            <option key={voice.name} value={voice.name}>
                              {voice.name} · {voice.gender === "female" ? "жен." : voice.gender === "male" ? "муж." : "нейтр."}
                              {voice.ru ? " · рус." : ""}
                            </option>
                          ))}
                        </select>
                      </Row>
                    )}
                  </>
                ) : (
                  <p className="text-xs text-stone-600">{stage.local}</p>
                )}
              </div>
            );
          })}

          <details className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
            <summary className="cursor-pointer text-sm text-stone-300">Тонкая настройка карты</summary>
            <div className="mt-3 space-y-3">
              <Row label="Контекст рассказчика" hint="Больше контекст — больше занято памяти под кэш внимания.">
                <input
                  type="number"
                  step={1024}
                  min={2048}
                  value={runtime.narratorCtx}
                  onChange={(event) => patch("narratorCtx", Number(event.target.value) || 2048)}
                  className={box}
                />
              </Row>
              <Row label="Слои на карте" hint="−1 — все слои на карте.">
                <input
                  type="number"
                  value={runtime.narratorGpuLayers}
                  onChange={(event) => patch("narratorGpuLayers", Number(event.target.value))}
                  className={box}
                />
              </Row>
              <Row label="Шагов на кадр" hint="Меньше шагов — быстрее кадр, но грубее.">
                <input
                  type="number"
                  min={1}
                  max={50}
                  value={runtime.imageSteps}
                  onChange={(event) => patch("imageSteps", Number(event.target.value) || 1)}
                  className={box}
                />
              </Row>
              <Row label="Сила промпта (CFG)">
                <input
                  type="number"
                  step={0.1}
                  min={0}
                  max={12}
                  value={runtime.imageCfg}
                  onChange={(event) => patch("imageCfg", Number(event.target.value))}
                  className={box}
                />
              </Row>
              <Toggle
                label="Выгружать веса кадра в память"
                checked={runtime.imageOffloadToCpu}
                onChange={(value) => patch("imageOffloadToCpu", value)}
              />
              <Toggle
                label="Озвучка включена"
                checked={runtime.ttsEnabled}
                onChange={(value) => patch("ttsEnabled", value)}
              />
              <Toggle
                label="Озвучка параллельно с кадром"
                checked={runtime.ttsParallel}
                onChange={(value) => patch("ttsParallel", value)}
              />
              <Row label="Секунд эталона голоса" hint="Дольше эталон — точнее тембр, дольше подготовка.">
                <input
                  type="number"
                  min={3}
                  max={30}
                  value={runtime.ttsReferenceSeconds}
                  onChange={(event) => patch("ttsReferenceSeconds", Number(event.target.value) || 3)}
                  className={box}
                />
              </Row>
              <Toggle
                label="Распознавание речи на карте"
                checked={runtime.asrOnGpu}
                onChange={(value) => patch("asrOnGpu", value)}
              />
            </div>
          </details>

          {note && <p className="text-xs leading-relaxed text-stone-500">{note}</p>}
        </div>
      )}
    </section>
  );
}
