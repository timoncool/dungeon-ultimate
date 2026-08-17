import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronRight, Loader2 } from "lucide-react";

// Настройки движков и облака.
//
// Всё это уже умел сервер, но дотянуться до него можно было только правкой файла: в игре
// не было ни выбора модели, ни переключателя «на карте / в облаке», ни размера контекста.
// Панель закрывает этот разрыв — четыре стадии хода переключаются по отдельности, поэтому
// играть можно и вовсе без видеокарты, и наполовину.

type Runtime = {
  /// Общий выбор устройства для локальных стадий: auto | gpu | cpu.
  localBackend: string;
  narratorBackend: string;
  imageBackend: string;
  ttsBackend: string;
  asrBackend: string;
  asrEngine: string;
  narratorCtx: number;
  narratorGpuLayers: number;
  imageSteps: number;
  imageCfg: number;
  imageOffloadToCpu: boolean;
  imageMaxVram: string;
  ttsEnabled: boolean;
  ttsParallel: boolean;
  ttsReferenceSeconds: number;
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
  /// Считается ли стадия на процессоре. У кадра и озвучки — нет: там это десятки минут на
  /// картинку и отставание голоса от чтения, то есть выбор, которым нельзя пользоваться.
  cpu: boolean;
  title: string;
  kind: string;
  on: keyof Runtime;
  model: keyof Runtime;
  /// Поле выбора устройства для локального счёта: пусто — как у всех.
  backend: keyof Runtime;
  local: string;
};

/// Где считается стадия. Три положения вместо двух: раньше «на карте» означало и карту, и
/// процессор — игрок не понимал, чем именно считается ход, и жаловался, что «одно на
/// процессоре, другое на карте».
type Where = "gpu" | "cpu" | "cloud";

const STAGES: Stage[] = [
  { key: "narrator", cpu: true, title: "Рассказчик", kind: "text", on: "openrouterNarratorOn", model: "openrouterNarratorModel", backend: "narratorBackend", local: "Гемма" },
  { key: "image", cpu: false, title: "Кадр", kind: "image", on: "openrouterImageOn", model: "openrouterImageModel", backend: "imageBackend", local: "Krea" },
  { key: "tts", cpu: false, title: "Озвучка", kind: "tts", on: "openrouterTtsOn", model: "openrouterTtsModel", backend: "ttsBackend", local: "Higgs" },
  { key: "asr", cpu: true, title: "Речь в текст", kind: "asr", on: "openrouterAsrOn", model: "openrouterAsrModel", backend: "asrBackend", local: "Parakeet" },
];

const WHERE_OPTIONS: Array<{ value: Where; label: string }> = [
  { value: "gpu", label: "Карта" },
  { value: "cpu", label: "Процессор" },
  { value: "cloud", label: "Облако" },
];

// Готовые раскладки. Модели не выдуманы — это те, на которых игра проверена вживую:
// картинку Gemini рисует за десяток секунд, речь Google озвучивает за пять, DeepSeek
// уверенно держит структурные проходы, Whisper — привычный выбор для расшифровки.
const PRESETS: Array<{ id: string; title: string; note: string; patch: Partial<Runtime> }> = [
  {
    id: "local",
    title: "Мощная карта — всё локально",
    note: "от 12 ГБ памяти: ничего не уходит наружу",
    patch: {
      openrouterNarratorOn: false,
      openrouterImageOn: false,
      openrouterTtsOn: false,
      openrouterAsrOn: false,
    },
  },
  {
    id: "cloud",
    title: "Без видеокарты — всё в облаке",
    note: "пойдёт на любом ноутбуке, нужен ключ",
    patch: {
      openrouterNarratorOn: true,
      openrouterNarratorModel: "deepseek/deepseek-v4-pro",
      openrouterImageOn: true,
      openrouterImageModel: "google/gemini-3.1-flash-image",
      openrouterTtsOn: true,
      openrouterTtsModel: "x-ai/grok-voice-tts-1.0",
      openrouterTtsVoice: "ara",
      openrouterAsrOn: true,
      openrouterAsrModel: "openai/whisper-large-v3",
    },
  },
  {
    id: "images-cloud",
    title: "Средняя карта — картинки в облаке",
    note: "самое тяжёлое наружу, остальное считает карта",
    patch: {
      openrouterNarratorOn: false,
      openrouterImageOn: true,
      openrouterImageModel: "google/gemini-3.1-flash-image",
      openrouterTtsOn: false,
      openrouterAsrOn: false,
    },
  },
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

  // Что сервер решил на самом деле. Настройки показывают выбор, а это — итог: пустой выбор
  // стадии подменяется общим, «как есть» превращается в конкретное железо, облако отменяет
  // и то, и другое. Игроки жаловались, что непонятно, чем считается ход, — теперь видно.
  const [decided, setDecided] = useState<Record<string, string>>({});
  useEffect(() => {
    let alive = true;
    const read = async () => {
      try {
        const answer = await fetch("/api/where", { cache: "no-store" });
        const data = (await answer.json()) as { stages?: Record<string, { where: string }> };
        if (!alive || !data.stages) return;
        setDecided(
          Object.fromEntries(Object.entries(data.stages).map(([key, value]) => [key, value.where])),
        );
      } catch {
        // не ответил — просто не показываем итог
      }
    };
    void read();
    const timer = window.setInterval(read, 4000);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [runtime]);

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

          <div className="space-y-1.5">
            <span className="block text-xs font-medium uppercase text-stone-500">Готовые раскладки</span>
            <div className="grid gap-1.5">
              {PRESETS.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  disabled={saving || (preset.id !== "local" && !keySet)}
                  onClick={() => {
                    setRuntime((current) => (current ? { ...current, ...preset.patch } : current));
                    void save(preset.patch);
                  }}
                  className="rounded border border-stone-800 bg-stone-950 px-3 py-2 text-left hover:border-amber-700/60 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  <span className="block text-sm text-stone-200">{preset.title}</span>
                  <span className="block text-[11px] leading-relaxed text-stone-600">{preset.note}</span>
                </button>
              ))}
            </div>
            {!keySet && (
              <p className="text-[11px] leading-relaxed text-stone-600">
                Для облачных раскладок нужен ключ OpenRouter — вставь его выше.
              </p>
            )}
          </div>

          <div className="space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-3">
            <div className="flex items-center justify-between gap-2">
              <span className="truncate text-sm text-stone-200">Считать по умолчанию</span>
              <div className="flex shrink-0 overflow-hidden rounded border border-stone-800">
                {[
                  { value: "auto", label: "Как есть" },
                  { value: "gpu", label: "Карта" },
                  { value: "cpu", label: "Процессор" },
                ].map((option) => (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => {
                      // Общий выбор действует на стадии, у которых нет своего: иначе он бы
                      // не менял ничего и выглядел сломанным.
                      patch("localBackend", option.value);
                      for (const stage of STAGES) {
                        patch(stage.backend, "");
                      }
                    }}
                    className={`whitespace-nowrap px-2 py-1 text-xs ${
                      (runtime.localBackend || "auto") === option.value
                        ? "bg-amber-200 text-stone-950"
                        : "text-stone-400 hover:text-stone-200"
                    }`}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>
            <p className="text-[11px] leading-relaxed text-stone-600">
              «Как есть» — карта, если она в системе есть, иначе процессор. Любую стадию
              ниже можно увести отдельно. Кадр и озвучка считаются только картой: на
              процессоре кадр рисуется десятки минут, а голос не поспевает за чтением.
            </p>
          </div>

          {STAGES.map((stage) => {
            const inCloud = runtime[stage.on] === true;
            const list = models[stage.kind] ?? [];
            const chosen = String(runtime[stage.model] ?? "");
            // Что выбрано сейчас: облако важнее устройства, оно отменяет локальный счёт.
            const chosenWhere: Where = inCloud
              ? "cloud"
              : (String(runtime[stage.backend] || runtime.localBackend || "auto") === "cpu"
                  ? "cpu"
                  : "gpu");
            const setWhere = (where: Where) => {
              if (where === "cloud") {
                patch(stage.on, true);
                return;
              }
              patch(stage.on, false);
              patch(stage.backend, where);
            };
            return (
              <div key={stage.key} className="space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-sm text-stone-200">{stage.title}</span>
                  {/* Три положения в одну строку: перенос читался бы как отдельные кнопки. */}
                  <div className="flex shrink-0 overflow-hidden rounded border border-stone-800">
                    {WHERE_OPTIONS.filter(
                      (option) => option.value !== "cpu" || stage.cpu,
                    ).map((option) => (
                      <button
                        key={option.value}
                        type="button"
                        disabled={option.value === "cloud" && !keySet}
                        onClick={() => setWhere(option.value)}
                        className={`whitespace-nowrap px-2 py-1 text-xs ${
                          chosenWhere === option.value
                            ? "bg-amber-200 text-stone-950"
                            : "text-stone-400 hover:text-stone-200 disabled:text-stone-700"
                        }`}
                      >
                        {option.label}
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
                  <p className="text-xs text-stone-600">
                    {stage.key === "asr" && (
                      <span className="mb-1 block">
                        <select
                          value={runtime.asrEngine || "parakeet"}
                          onChange={(event) => patch("asrEngine", event.target.value)}
                          className={box}
                        >
                          <option value="parakeet">Parakeet — быстрый, работает сразу</option>
                          <option value="whisper">Whisper large-v3 — точнее, нужна докачка</option>
                        </select>
                      </span>
                    )}
                    {stage.key === "narrator" && chosenWhere === "cpu" && (
                      <span className="mb-1 block text-amber-500/80">
                        Очень медленно: около полутора минут на отрывок против секунд на
                        карте. Замерено — 14 знаков в секунду.
                      </span>
                    )}
                    {stage.local} · считает{" "}
                    {decided[stage.key] === "cpu"
                      ? "процессор"
                      : decided[stage.key] === "gpu"
                        ? "видеокарта"
                        : chosenWhere === "cpu"
                          ? "процессор"
                          : "видеокарта"}
                  </p>
                )}
              </div>
            );
          })}

          <details className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
            <summary className="cursor-pointer text-sm text-stone-300">Железо: память и слои</summary>
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
              <Row label="Слоёв рассказчика на карте" hint="−1 — все. Не действует, когда рассказчик считается процессором.">
                <input
                  type="number"
                  value={runtime.narratorGpuLayers}
                  onChange={(event) => patch("narratorGpuLayers", Number(event.target.value))}
                  className={box}
                />
              </Row>
              <Toggle
                label="Держать веса кадра в оперативной памяти"
                checked={runtime.imageOffloadToCpu}
                onChange={(value) => patch("imageOffloadToCpu", value)}
              />
              <p className="text-[11px] leading-relaxed text-stone-600">
                Экономит память видеокарты, считает всё равно она. Где именно считать —
                выбирается выше, у стадии «Кадр».
              </p>
              <p className="text-[11px] leading-relaxed text-stone-600">
                Настройки самой озвучки — в разделе «Голос», отрисовки кадра — в «Картинках».
                Здесь остаётся только железо.
              </p>
            </div>
          </details>

          {note && <p className="text-xs leading-relaxed text-stone-500">{note}</p>}
        </div>
      )}
    </section>
  );
}
