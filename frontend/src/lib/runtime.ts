import { useCallback, useEffect, useState } from "react";

/// Настройки движка — общие на всё приложение, в отличие от настроек истории.
///
/// Живут они на сервере в одном файле, но касаются разных разделов: где считать — это
/// «Движки», шаги и сила промпта — «Картинки», громкость и параллельность — «Озвучка».
/// Раньше всё это лежало одной кучей в одной панели, потому что дотянуться до сервера
/// умела только она. Хук снимает это ограничение: раздел берёт ровно те поля, которые
/// относятся к его смыслу.
export type EngineRuntime = {
  /// Где считать локальные стадии: auto | gpu | cpu. Пусто у стадии — «как общий».
  localBackend: string;
  narratorBackend: string;
  imageBackend: string;
  ttsBackend: string;
  asrBackend: string;
  /// Каким движком распознавать речь: parakeet | whisper.
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

export type RuntimeHandle = {
  runtime: EngineRuntime | null;
  /// Ключ провайдера задан. Сам ключ сервер наружу не отдаёт.
  keySet: boolean;
  saving: boolean;
  note: string;
  /// Изменить одно поле. Правка сливается с сохранённым, остальное не трогается.
  patch: <K extends keyof EngineRuntime>(key: K, value: EngineRuntime[K]) => void;
  /// Изменить несколько полей разом — для готовых раскладок.
  apply: (patch: Partial<EngineRuntime>) => void;
};

/// Прочитать настройки движка и уметь их менять.
export function useEngineRuntime(): RuntimeHandle {
  const [runtime, setRuntime] = useState<EngineRuntime | null>(null);
  const [keySet, setKeySet] = useState(false);
  const [saving, setSaving] = useState(false);
  const [note, setNote] = useState("");

  const reload = useCallback(async () => {
    const data = await (await fetch("/api/runtime", { cache: "no-store" })).json();
    setRuntime(data.runtime);
    setKeySet(Boolean(data.openrouterKeySet));
  }, []);

  useEffect(() => {
    void reload().catch(() => setNote("Настройки движков не читаются."));
  }, [reload]);

  const apply = useCallback(
    (patch: Partial<EngineRuntime>) => {
      // Переключатель двигаем сразу: ждать ответа сервера — значит показывать игроку
      // залипающую кнопку. Не сохранилось — вернём то, что на сервере.
      setRuntime((current) => (current ? { ...current, ...patch } : current));
      setSaving(true);
      setNote("");
      void (async () => {
        try {
          const response = await fetch("/api/runtime", {
            method: "PUT",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(patch),
          });
          if (!response.ok) throw new Error(await response.text());
          await reload();
          // Перезапуск не нужен: сервер перечитывает настройки на каждом запуске движка.
          setNote("Сохранено — действует со следующего хода.");
        } catch (error) {
          setNote(`Не сохранилось: ${error instanceof Error ? error.message : String(error)}`);
          await reload().catch(() => {});
        } finally {
          setSaving(false);
        }
      })();
    },
    [reload],
  );

  const patch = useCallback(
    <K extends keyof EngineRuntime>(key: K, value: EngineRuntime[K]) =>
      apply({ [key]: value } as Partial<EngineRuntime>),
    [apply],
  );

  return { runtime, keySet, saving, note, patch, apply };
}
