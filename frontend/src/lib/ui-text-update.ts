import type { Language } from "@/lib/types";

// Экран обновления (CONTRACT, «The update screen»): подписи и причины изменений. Причины приходят с сервера кодом
// (crates/du-server/src/update_screen.rs, CHANGES), текст — здесь, на языке окна.

export type UpdateText = {
  kicker: string;
  title: string;
  subtitle: string;
  updated: string;
  added: string;
  retired: string;
  apply: string;
  later: string;
  download: string;
  freed: string;
  nothing: string;
  done: string;
  open: string;
  reasons: Record<string, string>;
};

const RU: UpdateText = {
  kicker: "ОБНОВЛЕНИЕ",
  title: "Что изменилось в движках и моделях",
  subtitle: "Этот выпуск обновляет то, что игра скачивает. Одна кнопка докачает новое и потом уберёт ненужное — прежнее удаляется, только когда замена на месте.",
  updated: "Обновится",
  added: "Новое",
  retired: "Больше не нужно",
  apply: "Обновить",
  later: "Позже",
  download: "скачать {size}",
  freed: "освободится {size}",
  nothing: "В этом выпуске менять нечего.",
  done: "Обновление готово",
  open: "Что изменилось в этом выпуске",
  reasons: {
    llama_newer: "новая сборка llama.cpp: быстрее и с поддержкой новых моделей",
    narrator_old_cards: "рассказчик на старых картах NVIDIA через CUDA 12",
    voice_server: "голос и CrisperWhisper теперь в сервере audio.cpp — быстрее и устойчивее",
    voice_quant: "лучший квант голоса для памяти вашей карты: чище звук",
    crisperwhisper: "CrisperWhisper вместо faster-whisper",
    parakeet_onnx_gone: "Parakeet теперь работает без ONNX Runtime",
    ort_gone: "ONNX Runtime больше не нужен",
    faster_whisper_gone: "faster-whisper заменён CrisperWhisper",
    higgs_library_gone: "прежняя библиотека голоса: её заменил сервер audio.cpp",
    references_moved: "прежние вырезки эталонов голосов: теперь эталон берётся по словам и лежит в .work",
    newer: "новая версия",
    unused: "больше не используется",
  },
};

const EN: UpdateText = {
  kicker: "UPDATE",
  title: "What changed in the engines and models",
  subtitle: "This release updates what the game downloads. One button fetches the new parts and then removes what is no longer needed — nothing old goes before its replacement is in place.",
  updated: "Updated",
  added: "New",
  retired: "No longer needed",
  apply: "Update",
  later: "Later",
  download: "download {size}",
  freed: "frees {size}",
  nothing: "Nothing to change in this release.",
  done: "The update is done",
  open: "What changed in this release",
  reasons: {
    llama_newer: "a newer llama.cpp build: faster and with support for new models",
    narrator_old_cards: "the narrator on older NVIDIA cards through CUDA 12",
    voice_server: "voice and CrisperWhisper now run in the audio.cpp server — faster and steadier",
    voice_quant: "the best voice quant for your card's memory: cleaner sound",
    crisperwhisper: "CrisperWhisper instead of faster-whisper",
    parakeet_onnx_gone: "Parakeet now runs without ONNX Runtime",
    ort_gone: "ONNX Runtime is no longer needed",
    faster_whisper_gone: "faster-whisper is replaced by CrisperWhisper",
    higgs_library_gone: "the former voice library: the audio.cpp server took its place",
    references_moved: "former voice reference cuts: the reference is now taken by words and kept in .work",
    newer: "a newer version",
    unused: "no longer used",
  },
};

const ES: UpdateText = {
  kicker: "ACTUALIZACIÓN",
  title: "Qué cambió en los motores y modelos",
  subtitle: "Esta versión actualiza lo que el juego descarga. Un botón trae lo nuevo y luego quita lo que ya no hace falta: nada antiguo se borra antes de que su reemplazo esté listo.",
  updated: "Se actualiza",
  added: "Nuevo",
  retired: "Ya no hace falta",
  apply: "Actualizar",
  later: "Más tarde",
  download: "descargar {size}",
  freed: "libera {size}",
  nothing: "Nada que cambiar en esta versión.",
  done: "La actualización está lista",
  open: "Qué cambió en esta versión",
  reasons: {
    llama_newer: "una versión más nueva de llama.cpp: más rápida y con soporte de modelos nuevos",
    narrator_old_cards: "el narrador en tarjetas NVIDIA antiguas con CUDA 12",
    voice_server: "la voz y CrisperWhisper ahora van en el servidor de audio.cpp: más rápidos y estables",
    voice_quant: "la mejor cuantización de voz para la memoria de tu tarjeta: sonido más limpio",
    crisperwhisper: "CrisperWhisper en lugar de faster-whisper",
    parakeet_onnx_gone: "Parakeet ya funciona sin ONNX Runtime",
    ort_gone: "ONNX Runtime ya no hace falta",
    faster_whisper_gone: "faster-whisper se sustituye por CrisperWhisper",
    higgs_library_gone: "la antigua biblioteca de voz: la sustituyó el servidor de audio.cpp",
    references_moved: "recortes antiguos de referencias de voz: ahora la referencia se toma por palabras y se guarda en .work",
    newer: "una versión más nueva",
    unused: "ya no se usa",
  },
};

const FR: UpdateText = {
  kicker: "MISE À JOUR",
  title: "Ce qui a changé dans les moteurs et les modèles",
  subtitle: "Cette version met à jour ce que le jeu télécharge. Un bouton récupère le nouveau puis retire ce qui ne sert plus — rien d’ancien ne part avant que son remplaçant soit en place.",
  updated: "Mis à jour",
  added: "Nouveau",
  retired: "Plus nécessaire",
  apply: "Mettre à jour",
  later: "Plus tard",
  download: "télécharger {size}",
  freed: "libère {size}",
  nothing: "Rien à changer dans cette version.",
  done: "La mise à jour est terminée",
  open: "Ce qui a changé dans cette version",
  reasons: {
    llama_newer: "une version plus récente de llama.cpp : plus rapide et compatible avec de nouveaux modèles",
    narrator_old_cards: "le narrateur sur les anciennes cartes NVIDIA via CUDA 12",
    voice_server: "la voix et CrisperWhisper tournent désormais dans le serveur audio.cpp — plus rapides et plus stables",
    voice_quant: "la meilleure quantification de voix pour la mémoire de votre carte : un son plus propre",
    crisperwhisper: "CrisperWhisper au lieu de faster-whisper",
    parakeet_onnx_gone: "Parakeet fonctionne désormais sans ONNX Runtime",
    ort_gone: "ONNX Runtime n’est plus nécessaire",
    faster_whisper_gone: "faster-whisper est remplacé par CrisperWhisper",
    higgs_library_gone: "l’ancienne bibliothèque de voix : le serveur audio.cpp l’a remplacée",
    references_moved: "anciennes découpes de références vocales : la référence est désormais prise par mots et rangée dans .work",
    newer: "une version plus récente",
    unused: "n’est plus utilisé",
  },
};

const DE: UpdateText = {
  kicker: "UPDATE",
  title: "Was sich bei Engines und Modellen geändert hat",
  subtitle: "Diese Version aktualisiert, was das Spiel herunterlädt. Ein Knopf holt das Neue und entfernt dann, was nicht mehr gebraucht wird — nichts Altes geht, bevor sein Ersatz da ist.",
  updated: "Wird aktualisiert",
  added: "Neu",
  retired: "Nicht mehr nötig",
  apply: "Aktualisieren",
  later: "Später",
  download: "{size} herunterladen",
  freed: "{size} frei",
  nothing: "In dieser Version gibt es nichts zu ändern.",
  done: "Das Update ist fertig",
  open: "Was sich in dieser Version geändert hat",
  reasons: {
    llama_newer: "ein neuerer llama.cpp-Build: schneller und mit Unterstützung neuer Modelle",
    narrator_old_cards: "der Erzähler auf älteren NVIDIA-Karten über CUDA 12",
    voice_server: "Stimme und CrisperWhisper laufen jetzt im audio.cpp-Server — schneller und stabiler",
    voice_quant: "die beste Stimm-Quantisierung für den Speicher deiner Karte: saubererer Klang",
    crisperwhisper: "CrisperWhisper statt faster-whisper",
    parakeet_onnx_gone: "Parakeet läuft jetzt ohne ONNX Runtime",
    ort_gone: "ONNX Runtime wird nicht mehr gebraucht",
    faster_whisper_gone: "faster-whisper wird durch CrisperWhisper ersetzt",
    higgs_library_gone: "die frühere Stimmbibliothek: der audio.cpp-Server hat sie ersetzt",
    references_moved: "frühere Ausschnitte der Stimmreferenzen: die Referenz wird jetzt nach Wörtern genommen und in .work abgelegt",
    newer: "eine neuere Version",
    unused: "wird nicht mehr verwendet",
  },
};

const ZH: UpdateText = {
  kicker: "更新",
  title: "引擎和模型有哪些变化",
  subtitle: "此版本更新了游戏下载的内容。一个按钮下载新的部分，然后删除不再需要的内容——替代品就位之前，旧的不会被删除。",
  updated: "将更新",
  added: "新增",
  retired: "不再需要",
  apply: "更新",
  later: "稍后",
  download: "下载 {size}",
  freed: "释放 {size}",
  nothing: "此版本没有需要更改的内容。",
  done: "更新完成",
  open: "此版本有哪些变化",
  reasons: {
    llama_newer: "更新的 llama.cpp 构建：更快，并支持新模型",
    narrator_old_cards: "讲述者通过 CUDA 12 在旧款 NVIDIA 显卡上运行",
    voice_server: "语音和 CrisperWhisper 现在运行在 audio.cpp 服务器中——更快更稳定",
    voice_quant: "适合你的显卡显存的最佳语音量化：声音更干净",
    crisperwhisper: "用 CrisperWhisper 代替 faster-whisper",
    parakeet_onnx_gone: "Parakeet 现在不再需要 ONNX Runtime",
    ort_gone: "不再需要 ONNX Runtime",
    faster_whisper_gone: "faster-whisper 已被 CrisperWhisper 取代",
    higgs_library_gone: "旧的语音库：已被 audio.cpp 服务器取代",
    references_moved: "旧的语音参考片段：参考现在按词选取并保存在 .work 中",
    newer: "更新的版本",
    unused: "不再使用",
  },
};

const JA: UpdateText = {
  kicker: "アップデート",
  title: "エンジンとモデルの変更点",
  subtitle: "このリリースでは、ゲームがダウンロードするものが更新されます。ボタンひとつで新しいものを取得し、そのあと不要なものを削除します。代わりが揃うまで古いものは消えません。",
  updated: "更新",
  added: "新規",
  retired: "不要になったもの",
  apply: "更新する",
  later: "あとで",
  download: "{size} をダウンロード",
  freed: "{size} 空く",
  nothing: "このリリースで変更するものはありません。",
  done: "アップデートが完了しました",
  open: "このリリースの変更点",
  reasons: {
    llama_newer: "新しい llama.cpp ビルド：より速く、新しいモデルに対応",
    narrator_old_cards: "古い NVIDIA カードで CUDA 12 を使う語り手",
    voice_server: "音声と CrisperWhisper は audio.cpp サーバーで動くようになり、より速く安定",
    voice_quant: "カードのメモリに合った最良の音声量子化：よりクリアな音",
    crisperwhisper: "faster-whisper の代わりに CrisperWhisper",
    parakeet_onnx_gone: "Parakeet は ONNX Runtime なしで動くようになった",
    ort_gone: "ONNX Runtime はもう不要",
    faster_whisper_gone: "faster-whisper は CrisperWhisper に置き換えられた",
    higgs_library_gone: "以前の音声ライブラリ：audio.cpp サーバーに置き換えられた",
    references_moved: "以前の音声リファレンスの切り抜き：リファレンスは単語単位で選ばれ .work に置かれる",
    newer: "新しいバージョン",
    unused: "使われなくなった",
  },
};

const BY_LANGUAGE: Record<Language, UpdateText> = { ru: RU, en: EN, es: ES, fr: FR, de: DE, zh: ZH, ja: JA };

export function updateText(language: Language | undefined): UpdateText {
  return BY_LANGUAGE[language ?? "ru"] ?? RU;
}
