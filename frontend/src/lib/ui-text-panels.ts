import type { Language } from "@/lib/types";

/// Подписи панелей: движки и облако, что скачать, монитор карты, книга.
///
/// Держим их отдельно от подписей главного экрана: там короткие кнопки, здесь — длинные
/// пояснения, и вместе они превращаются в простыню, в которой не найти нужное.
export type PanelText = {
  // Стадии хода и где они считаются
  narrator: string;
  narratorModel: string;
  frame: string;
  narration: string;
  speechToText: string;
  onCard: string;
  onCpu: string;
  inCloud: string;
  cpuWord: string;
  gpuWord: string;

  // Готовые раскладки
  presets: string;
  presetStrongCard: string;
  presetStrongCardNote: string;
  presetNoCard: string;
  presetNoCardNote: string;
  presetMidCard: string;
  presetMidCardNote: string;
  presetsNeedKey: string;
  computeByDefault: string;

  // Ключ и сохранение
  keySet: string;
  keyMissing: string;
  replaceKey: string;
  save: string;
  saved: string;
  settingsUnreadable: string;
  allOnOwnCard: string;

  // Голоса
  voicesRussianMarked: string;
  voiceFemale: string;
  voiceMale: string;
  voiceNeutral: string;
  voiceRussianSuffix: string;
  asIs: string;

  // Железо
  hardwareSection: string;
  narratorContext: string;
  narratorContextNote: string;
  narratorLayers: string;
  narratorLayersNote: string;
  keepFrameWeightsInRam: string;

  // Пояснения и прочие панели
  keyHint: string;
  asIsHint: string;
  componentsUnreadable: string;
  ready: string;
  requiredNote: string;
  recommendedNote: string;
  optionalNote: string;
  download: string;
  monitorDetach: string;
  monitorAttach: string;
  power: string;
  process: string;
  item: string;
  turnRunning: string;
  prevPage: string;
  readSpread: string;
  nextPage: string;

  // Распознавание речи
  asrParakeet: string;
  asrWhisper: string;
  // Остатки текста из кода (ревью 2026-09)
  notSaved: string;
  cpuSlowNote: string;
  computesWord: string;
  imageWorkerNote: string;
  hardwareOnlyNote: string;
  gbUnit: string;
  wattUnit: string;
  resources: string;
  downloadFailed: string;
  missingRequiredCount: string;
  optionalAvailableCount: string;
  downloadRequired: string;
  allRequiredPresent: string;
  resumableNote: string;
  bookEmpty: string;
  readAloud: string;
  flipPages: string;
};

const RU: PanelText = {
  narrator: "Рассказчик",
  narratorModel: "Гемма",
  frame: "Кадр",
  narration: "Озвучка",
  speechToText: "Речь в текст",
  onCard: "Карта",
  onCpu: "Процессор",
  inCloud: "Облако",
  cpuWord: "процессор",
  gpuWord: "видеокарта",

  presets: "Готовые раскладки",
  presetStrongCard: "Мощная карта — всё локально",
  presetStrongCardNote: "от 12 ГБ памяти: ничего не уходит наружу",
  presetNoCard: "Без видеокарты — всё в облаке",
  presetNoCardNote: "пойдёт на любом ноутбуке, нужен ключ",
  presetMidCard: "Средняя карта — картинки в облаке",
  presetMidCardNote: "самое тяжёлое наружу, остальное считает карта",
  presetsNeedKey: "Для облачных раскладок нужен ключ OpenRouter — вставь его выше.",
  computeByDefault: "Считать по умолчанию",

  keySet: "задан",
  keyMissing: "не задан",
  replaceKey: "Заменить ключ",
  save: "Сохранить",
  saved: "Сохранено — действует со следующего хода.",
  settingsUnreadable: "Настройки движков не читаются.",
  allOnOwnCard: "Всё считает своя карта",

  voicesRussianMarked: "Отмечены голоса, которые тянут русскую речь.",
  voiceFemale: "жен.",
  voiceMale: "муж.",
  voiceNeutral: "нейтр.",
  voiceRussianSuffix: " · рус.",
  asIs: "Как есть",

  hardwareSection: "Железо: память и слои",
  narratorContext: "Контекст рассказчика",
  narratorContextNote: "Больше контекст — больше занято памяти под кэш внимания.",
  narratorLayers: "Слоёв рассказчика на карте",
  narratorLayersNote: "−1 — все. Не действует, когда рассказчик считается процессором.",
  keepFrameWeightsInRam: "Держать веса кадра в оперативной памяти",

  keyHint: "Ключ OpenRouter {state}. Без него облачные стадии не включаются и всё считает своя карта.",
  asIsHint: "«Как есть» — карта, если она в системе есть, иначе процессор. Любую стадию ниже можно увести отдельно.",
  componentsUnreadable: "Список компонентов не читается.",
  ready: "Готово",
  requiredNote: " · без него игра не пойдёт",
  recommendedNote: " · рекомендуется",
  optionalNote: " · по желанию",
  download: "Скачать",
  monitorDetach: "Оторвать монитор ресурсов",
  monitorAttach: "Вернуть в шапку",
  power: "Питание",
  process: "Процесс",
  item: "Предмет",
  turnRunning: "Ход идёт…",
  prevPage: "Предыдущая страница",
  readSpread: "Озвучить открытый разворот",
  nextPage: "Следующая страница",

  asrParakeet: "Parakeet — быстрый, работает сразу",
  asrWhisper: "Whisper large-v3 — точнее, нужна докачка",
  notSaved: "Не сохранилось: {error}",
  cpuSlowNote: "Очень медленно: около полутора минут на отрывок против секунд на карте. Замерено — 14 знаков в секунду.",
  computesWord: "считает",
  imageWorkerNote: "Экономит память видеокарты, считает всё равно она. Где именно считать — выбирается выше, у стадии «{frame}».",
  hardwareOnlyNote: "Настройки самой озвучки — в разделе «Голос», отрисовки кадра — в «Картинках». Здесь остаётся только железо.",
  gbUnit: "ГБ",
  wattUnit: "Вт",
  resources: "Ресурсы",
  downloadFailed: "Не скачалось: {error}",
  missingRequiredCount: "Не хватает главного: {n}",
  optionalAvailableCount: "Можно доставить: {n}",
  downloadRequired: "Скачать нужное — {size}",
  allRequiredPresent: "Всё главное на месте",
  resumableNote: "Всего не хватает {size}. Качается с докачкой: прерванная загрузка продолжится с того же места, а не начнётся заново.",
  bookEmpty: "Книга пока пуста — начни историю.",
  readAloud: "Озвучить",
  flipPages: "листай · {n}",
};

const EN: PanelText = {
  narrator: "Narrator",
  narratorModel: "Gemma",
  frame: "Frame",
  narration: "Narration",
  speechToText: "Speech to text",
  onCard: "Card",
  onCpu: "CPU",
  inCloud: "Cloud",
  cpuWord: "CPU",
  gpuWord: "graphics card",

  presets: "Ready-made layouts",
  presetStrongCard: "Strong card — everything local",
  presetStrongCardNote: "12 GB of memory and up: nothing leaves the machine",
  presetNoCard: "No graphics card — everything in the cloud",
  presetNoCardNote: "runs on any laptop, needs a key",
  presetMidCard: "Middling card — images in the cloud",
  presetMidCardNote: "the heaviest part goes out, the card does the rest",
  presetsNeedKey: "Cloud layouts need an OpenRouter key — paste it above.",
  computeByDefault: "Compute by default",

  keySet: "set",
  keyMissing: "not set",
  replaceKey: "Replace the key",
  save: "Save",
  saved: "Saved — takes effect from the next turn.",
  settingsUnreadable: "Engine settings cannot be read.",
  allOnOwnCard: "Everything runs on my card",

  voicesRussianMarked: "Marked voices actually speak Russian.",
  voiceFemale: "female",
  voiceMale: "male",
  voiceNeutral: "neutral",
  voiceRussianSuffix: " · RU",
  asIs: "As is",

  hardwareSection: "Hardware: memory and layers",
  narratorContext: "Narrator context",
  narratorContextNote: "A bigger context takes more memory for the attention cache.",
  narratorLayers: "Narrator layers on the card",
  narratorLayersNote: "−1 means all. Has no effect while the narrator runs on the CPU.",
  keepFrameWeightsInRam: "Keep frame weights in RAM",

  keyHint: "The OpenRouter key is {state}. Without it cloud stages stay off and everything runs on your card.",
  asIsHint: "«As is» means the card when there is one, otherwise the CPU. Any stage below can be moved on its own.",
  componentsUnreadable: "The component list cannot be read.",
  ready: "Done",
  requiredNote: " · the game will not run without it",
  recommendedNote: " · recommended",
  optionalNote: " · optional",
  download: "Download",
  monitorDetach: "Detach the resource monitor",
  monitorAttach: "Return to the header",
  power: "Power",
  process: "Process",
  item: "Item",
  turnRunning: "The turn is running…",
  prevPage: "Previous page",
  readSpread: "Read the open spread aloud",
  nextPage: "Next page",

  asrParakeet: "Parakeet — fast, works right away",
  asrWhisper: "Whisper large-v3 — more precise, needs a download",
  notSaved: "Not saved: {error}",
  cpuSlowNote: "Very slow: about a minute and a half per passage versus seconds on the GPU. Measured at 14 characters per second.",
  computesWord: "runs on",
  imageWorkerNote: "Saves GPU memory; the GPU still does the work. Where to compute is chosen above, at the “{frame}” stage.",
  hardwareOnlyNote: "Voice settings live in the “Voice” section, frame rendering in “Images”. Only the hardware stays here.",
  gbUnit: "GB",
  wattUnit: "W",
  resources: "Resources",
  downloadFailed: "Download failed: {error}",
  missingRequiredCount: "Essentials missing: {n}",
  optionalAvailableCount: "Available to add: {n}",
  downloadRequired: "Download the essentials — {size}",
  allRequiredPresent: "All essentials are in place",
  resumableNote: "{size} missing in total. Downloads resume: an interrupted download continues from where it stopped instead of starting over.",
  bookEmpty: "The book is still empty — start a story.",
  readAloud: "Read aloud",
  flipPages: "flip · {n}",
};

const ES: PanelText = {
  narrator: "Narrador",
  narratorModel: "Gemma",
  frame: "Fotograma",
  narration: "Locución",
  speechToText: "Voz a texto",
  onCard: "Tarjeta",
  onCpu: "CPU",
  inCloud: "Nube",
  cpuWord: "CPU",
  gpuWord: "tarjeta gráfica",

  presets: "Configuraciones listas",
  presetStrongCard: "Tarjeta potente: todo en local",
  presetStrongCardNote: "desde 12 GB de memoria: nada sale del equipo",
  presetNoCard: "Sin tarjeta gráfica: todo en la nube",
  presetNoCardNote: "va en cualquier portátil, hace falta una clave",
  presetMidCard: "Tarjeta media: imágenes en la nube",
  presetMidCardNote: "lo más pesado sale fuera, el resto lo hace la tarjeta",
  presetsNeedKey: "Las configuraciones en la nube necesitan una clave de OpenRouter: pégala arriba.",
  computeByDefault: "Calcular por defecto",

  keySet: "puesta",
  keyMissing: "sin poner",
  replaceKey: "Cambiar la clave",
  save: "Guardar",
  saved: "Guardado: se aplica desde el próximo turno.",
  settingsUnreadable: "No se pueden leer los ajustes de los motores.",
  allOnOwnCard: "Todo lo calcula mi tarjeta",

  voicesRussianMarked: "Las voces marcadas hablan ruso de verdad.",
  voiceFemale: "fem.",
  voiceMale: "masc.",
  voiceNeutral: "neutra",
  voiceRussianSuffix: " · RU",
  asIs: "Tal cual",

  hardwareSection: "Equipo: memoria y capas",
  narratorContext: "Contexto del narrador",
  narratorContextNote: "Más contexto ocupa más memoria para la caché de atención.",
  narratorLayers: "Capas del narrador en la tarjeta",
  narratorLayersNote: "−1 son todas. No hace nada mientras el narrador va por CPU.",
  keepFrameWeightsInRam: "Mantener los pesos del fotograma en RAM",

  keyHint: "La clave de OpenRouter está {state}. Sin ella las etapas en la nube no se activan y todo lo calcula tu tarjeta.",
  asIsHint: "«Tal cual» significa la tarjeta si la hay, si no la CPU. Cada etapa se puede mover por separado.",
  componentsUnreadable: "No se puede leer la lista de componentes.",
  ready: "Listo",
  requiredNote: " · sin esto el juego no arranca",
  recommendedNote: " · recomendado",
  optionalNote: " · opcional",
  download: "Descargar",
  monitorDetach: "Separar el monitor de recursos",
  monitorAttach: "Devolver a la cabecera",
  power: "Consumo",
  process: "Proceso",
  item: "Objeto",
  turnRunning: "El turno está en marcha…",
  prevPage: "Página anterior",
  readSpread: "Leer en voz alta la doble página",
  nextPage: "Página siguiente",

  asrParakeet: "Parakeet: rápido, funciona ya",
  asrWhisper: "Whisper large-v3: más preciso, hay que descargarlo",
  notSaved: "No se guardó: {error}",
  cpuSlowNote: "Muy lento: cerca de minuto y medio por pasaje frente a segundos en la tarjeta gráfica. Medido: 14 caracteres por segundo.",
  computesWord: "calcula en",
  imageWorkerNote: "Ahorra memoria de la tarjeta gráfica; el cálculo sigue haciéndolo ella. Dónde calcular se elige arriba, en la etapa «{frame}».",
  hardwareOnlyNote: "Los ajustes de la voz están en «Voz», los del fotograma en «Imágenes». Aquí solo queda el hardware.",
  gbUnit: "GB",
  wattUnit: "W",
  resources: "Recursos",
  downloadFailed: "No se descargó: {error}",
  missingRequiredCount: "Faltan esenciales: {n}",
  optionalAvailableCount: "Se pueden añadir: {n}",
  downloadRequired: "Descargar lo esencial: {size}",
  allRequiredPresent: "Todo lo esencial está listo",
  resumableNote: "Faltan {size} en total. Las descargas se reanudan: una descarga interrumpida continúa desde donde se quedó en lugar de empezar de nuevo.",
  bookEmpty: "El libro aún está vacío: empieza una historia.",
  readAloud: "Leer en voz alta",
  flipPages: "pasa página · {n}",
};

const FR: PanelText = {
  narrator: "Narrateur",
  narratorModel: "Gemma",
  frame: "Image",
  narration: "Lecture",
  speechToText: "Voix vers texte",
  onCard: "Carte",
  onCpu: "Processeur",
  inCloud: "Nuage",
  cpuWord: "processeur",
  gpuWord: "carte graphique",

  presets: "Configurations prêtes",
  presetStrongCard: "Carte puissante — tout en local",
  presetStrongCardNote: "à partir de 12 Go de mémoire : rien ne sort de la machine",
  presetNoCard: "Sans carte graphique — tout dans le nuage",
  presetNoCardNote: "tourne sur n'importe quel portable, il faut une clé",
  presetMidCard: "Carte moyenne — les images dans le nuage",
  presetMidCardNote: "le plus lourd part dehors, la carte fait le reste",
  presetsNeedKey: "Les configurations dans le nuage demandent une clé OpenRouter — colle-la plus haut.",
  computeByDefault: "Calculer par défaut",

  keySet: "définie",
  keyMissing: "absente",
  replaceKey: "Remplacer la clé",
  save: "Enregistrer",
  saved: "Enregistré — actif dès le prochain tour.",
  settingsUnreadable: "Impossible de lire les réglages des moteurs.",
  allOnOwnCard: "Tout est calculé par ma carte",

  voicesRussianMarked: "Les voix marquées parlent vraiment russe.",
  voiceFemale: "fém.",
  voiceMale: "masc.",
  voiceNeutral: "neutre",
  voiceRussianSuffix: " · RU",
  asIs: "Tel quel",

  hardwareSection: "Matériel : mémoire et couches",
  narratorContext: "Contexte du narrateur",
  narratorContextNote: "Plus de contexte, plus de mémoire prise par le cache d'attention.",
  narratorLayers: "Couches du narrateur sur la carte",
  narratorLayersNote: "−1 signifie toutes. Sans effet quand le narrateur tourne sur le processeur.",
  keepFrameWeightsInRam: "Garder les poids de l'image en mémoire vive",

  keyHint: "La clé OpenRouter est {state}. Sans elle, les étapes dans le nuage restent éteintes et tout passe par ta carte.",
  asIsHint: "« Tel quel » veut dire la carte si elle existe, sinon le processeur. Chaque étape se déplace séparément.",
  componentsUnreadable: "La liste des composants est illisible.",
  ready: "Prêt",
  requiredNote: " · sans lui le jeu ne démarre pas",
  recommendedNote: " · recommandé",
  optionalNote: " · au choix",
  download: "Télécharger",
  monitorDetach: "Détacher le moniteur de ressources",
  monitorAttach: "Remettre dans l'en-tête",
  power: "Puissance",
  process: "Processus",
  item: "Objet",
  turnRunning: "Le tour est en cours…",
  prevPage: "Page précédente",
  readSpread: "Lire la double page à voix haute",
  nextPage: "Page suivante",

  asrParakeet: "Parakeet — rapide, marche tout de suite",
  asrWhisper: "Whisper large-v3 — plus précis, à télécharger",
  notSaved: "Non enregistré : {error}",
  cpuSlowNote: "Très lent : environ une minute et demie par passage contre quelques secondes sur la carte graphique. Mesuré : 14 caractères par seconde.",
  computesWord: "calcule sur",
  imageWorkerNote: "Économise la mémoire de la carte graphique ; c'est toujours elle qui calcule. Où calculer se choisit plus haut, à l'étape « {frame} ».",
  hardwareOnlyNote: "Les réglages de la voix sont dans « Voix », ceux du rendu dans « Images ». Ici, il ne reste que le matériel.",
  gbUnit: "Go",
  wattUnit: "W",
  resources: "Ressources",
  downloadFailed: "Téléchargement échoué : {error}",
  missingRequiredCount: "Éléments essentiels manquants : {n}",
  optionalAvailableCount: "Disponibles en plus : {n}",
  downloadRequired: "Télécharger l'essentiel — {size}",
  allRequiredPresent: "Tout l'essentiel est là",
  resumableNote: "{size} manquants au total. Les téléchargements reprennent : un téléchargement interrompu continue là où il s'est arrêté au lieu de repartir de zéro.",
  bookEmpty: "Le livre est encore vide : commence une histoire.",
  readAloud: "Lire à voix haute",
  flipPages: "feuillette · {n}",
};

const DE: PanelText = {
  narrator: "Erzähler",
  narratorModel: "Gemma",
  frame: "Bild",
  narration: "Vorlesen",
  speechToText: "Sprache zu Text",
  onCard: "Karte",
  onCpu: "Prozessor",
  inCloud: "Cloud",
  cpuWord: "Prozessor",
  gpuWord: "Grafikkarte",

  presets: "Fertige Aufteilungen",
  presetStrongCard: "Starke Karte — alles lokal",
  presetStrongCardNote: "ab 12 GB Speicher: nichts verlässt den Rechner",
  presetNoCard: "Ohne Grafikkarte — alles in der Cloud",
  presetNoCardNote: "läuft auf jedem Laptop, braucht einen Schlüssel",
  presetMidCard: "Mittlere Karte — Bilder in der Cloud",
  presetMidCardNote: "das Schwerste geht nach draußen, den Rest macht die Karte",
  presetsNeedKey: "Cloud-Aufteilungen brauchen einen OpenRouter-Schlüssel — oben einfügen.",
  computeByDefault: "Standardmäßig rechnen",

  keySet: "gesetzt",
  keyMissing: "fehlt",
  replaceKey: "Schlüssel ersetzen",
  save: "Speichern",
  saved: "Gespeichert — gilt ab dem nächsten Zug.",
  settingsUnreadable: "Die Engine-Einstellungen sind nicht lesbar.",
  allOnOwnCard: "Alles rechnet die eigene Karte",

  voicesRussianMarked: "Markierte Stimmen sprechen wirklich Russisch.",
  voiceFemale: "weibl.",
  voiceMale: "männl.",
  voiceNeutral: "neutral",
  voiceRussianSuffix: " · RU",
  asIs: "Wie es ist",

  hardwareSection: "Hardware: Speicher und Schichten",
  narratorContext: "Kontext des Erzählers",
  narratorContextNote: "Mehr Kontext belegt mehr Speicher für den Aufmerksamkeits-Cache.",
  narratorLayers: "Erzähler-Schichten auf der Karte",
  narratorLayersNote: "−1 heißt alle. Ohne Wirkung, solange der Erzähler auf dem Prozessor läuft.",
  keepFrameWeightsInRam: "Bildgewichte im Arbeitsspeicher halten",

  keyHint: "Der OpenRouter-Schlüssel ist {state}. Ohne ihn bleiben Cloud-Stufen aus und alles rechnet die eigene Karte.",
  asIsHint: "«Wie es ist» heißt Karte, wenn eine da ist, sonst Prozessor. Jede Stufe lässt sich einzeln verschieben.",
  componentsUnreadable: "Die Komponentenliste ist nicht lesbar.",
  ready: "Fertig",
  requiredNote: " · ohne das läuft das Spiel nicht",
  recommendedNote: " · empfohlen",
  optionalNote: " · nach Wunsch",
  download: "Laden",
  monitorDetach: "Ressourcenmonitor ablösen",
  monitorAttach: "Zurück in die Kopfzeile",
  power: "Leistung",
  process: "Prozess",
  item: "Gegenstand",
  turnRunning: "Der Zug läuft…",
  prevPage: "Vorherige Seite",
  readSpread: "Die offene Doppelseite vorlesen",
  nextPage: "Nächste Seite",

  asrParakeet: "Parakeet — schnell, läuft sofort",
  asrWhisper: "Whisper large-v3 — genauer, muss geladen werden",
  notSaved: "Nicht gespeichert: {error}",
  cpuSlowNote: "Sehr langsam: etwa anderthalb Minuten pro Abschnitt gegenüber Sekunden auf der Grafikkarte. Gemessen: 14 Zeichen pro Sekunde.",
  computesWord: "rechnet auf",
  imageWorkerNote: "Spart Grafikspeicher; gerechnet wird trotzdem auf der Karte. Wo gerechnet wird, wählst du oben bei der Stufe „{frame}“.",
  hardwareOnlyNote: "Die Sprachausgabe stellst du unter „Stimme“ ein, das Bildrendering unter „Bilder“. Hier bleibt nur die Hardware.",
  gbUnit: "GB",
  wattUnit: "W",
  resources: "Ressourcen",
  downloadFailed: "Download fehlgeschlagen: {error}",
  missingRequiredCount: "Wesentliches fehlt: {n}",
  optionalAvailableCount: "Zusätzlich verfügbar: {n}",
  downloadRequired: "Das Nötige laden – {size}",
  allRequiredPresent: "Alles Wesentliche ist da",
  resumableNote: "Insgesamt fehlen {size}. Downloads werden fortgesetzt: ein abgebrochener Download macht dort weiter, wo er aufgehört hat, statt neu zu beginnen.",
  bookEmpty: "Das Buch ist noch leer – beginne eine Geschichte.",
  readAloud: "Vorlesen",
  flipPages: "blättern · {n}",
};

const ZH: PanelText = {
  narrator: "叙述者",
  narratorModel: "Gemma",
  frame: "画面",
  narration: "朗读",
  speechToText: "语音转文字",
  onCard: "显卡",
  onCpu: "处理器",
  inCloud: "云端",
  cpuWord: "处理器",
  gpuWord: "显卡",

  presets: "现成方案",
  presetStrongCard: "显卡强劲——全部在本机",
  presetStrongCardNote: "12 GB 显存起：什么都不外传",
  presetNoCard: "没有显卡——全部在云端",
  presetNoCardNote: "任何笔记本都能跑，需要密钥",
  presetMidCard: "显卡一般——画面放云端",
  presetMidCardNote: "最重的部分外发，其余交给显卡",
  presetsNeedKey: "云端方案需要 OpenRouter 密钥——请在上方填入。",
  computeByDefault: "默认在哪里计算",

  keySet: "已填",
  keyMissing: "未填",
  replaceKey: "更换密钥",
  save: "保存",
  saved: "已保存——下一回合生效。",
  settingsUnreadable: "读不到引擎设置。",
  allOnOwnCard: "全部由本机显卡计算",

  voicesRussianMarked: "标记过的声音确实会说俄语。",
  voiceFemale: "女",
  voiceMale: "男",
  voiceNeutral: "中性",
  voiceRussianSuffix: " · 俄",
  asIs: "保持原样",

  hardwareSection: "硬件：显存与层数",
  narratorContext: "叙述者上下文",
  narratorContextNote: "上下文越大，注意力缓存占用的显存越多。",
  narratorLayers: "叙述者放在显卡上的层数",
  narratorLayersNote: "−1 表示全部。叙述者走处理器时此项无效。",
  keepFrameWeightsInRam: "把画面权重留在内存里",

  keyHint: "OpenRouter 密钥{state}。没有它，云端阶段不会开启，一切都由本机显卡计算。",
  asIsHint: "「保持原样」是指有显卡就用显卡，否则用处理器。下面每个阶段都可以单独挪动。",
  componentsUnreadable: "读不到组件列表。",
  ready: "完成",
  requiredNote: " · 没有它游戏跑不起来",
  recommendedNote: " · 建议下载",
  optionalNote: " · 可选",
  download: "下载",
  monitorDetach: "把资源监视器拆出来",
  monitorAttach: "放回顶栏",
  power: "功耗",
  process: "进程",
  item: "物品",
  turnRunning: "回合进行中……",
  prevPage: "上一页",
  readSpread: "朗读当前跨页",
  nextPage: "下一页",

  asrParakeet: "Parakeet——快，装好即用",
  asrWhisper: "Whisper large-v3——更准，需要另外下载",
  notSaved: "未能保存：{error}",
  cpuSlowNote: "非常慢：每段约一分半钟，而显卡只需几秒。实测每秒 14 个字符。",
  computesWord: "运行于",
  imageWorkerNote: "节省显存，但仍由显卡计算。在哪里计算，请在上方“{frame}”阶段选择。",
  hardwareOnlyNote: "语音设置在“语音”栏，画面渲染在“图片”栏。这里只剩硬件。",
  gbUnit: "GB",
  wattUnit: "W",
  resources: "资源",
  downloadFailed: "下载失败：{error}",
  missingRequiredCount: "缺少必需项：{n}",
  optionalAvailableCount: "可选安装：{n}",
  downloadRequired: "下载必需项——{size}",
  allRequiredPresent: "必需项已齐全",
  resumableNote: "总共缺少 {size}。支持断点续传：中断的下载会从原处继续，而不是重新开始。",
  bookEmpty: "书还是空的——开始一个故事吧。",
  readAloud: "朗读",
  flipPages: "翻页 · {n}",
};

const JA: PanelText = {
  narrator: "語り手",
  narratorModel: "Gemma",
  frame: "絵",
  narration: "読み上げ",
  speechToText: "音声を文字に",
  onCard: "画板",
  onCpu: "処理装置",
  inCloud: "雲",
  cpuWord: "処理装置",
  gpuWord: "画板",

  presets: "出来合いの配分",
  presetStrongCard: "強い画板——すべて手元で",
  presetStrongCardNote: "12 GB 以上：何も外に出ない",
  presetNoCard: "画板なし——すべて雲の上で",
  presetNoCardNote: "どのノートでも動く。鍵が要る",
  presetMidCard: "並の画板——絵だけ雲の上",
  presetMidCardNote: "いちばん重い所を外へ、残りは画板が",
  presetsNeedKey: "雲の配分には OpenRouter の鍵が要る——上に貼ること。",
  computeByDefault: "既定でどこで計算するか",

  keySet: "あり",
  keyMissing: "なし",
  replaceKey: "鍵を差し替える",
  save: "保存",
  saved: "保存した——次の手番から効く。",
  settingsUnreadable: "エンジンの設定が読めない。",
  allOnOwnCard: "すべて手元の画板が計算する",

  voicesRussianMarked: "印のある声は本当にロシア語を話す。",
  voiceFemale: "女",
  voiceMale: "男",
  voiceNeutral: "中性",
  voiceRussianSuffix: " · 露",
  asIs: "そのまま",

  hardwareSection: "機械：記憶と層",
  narratorContext: "語り手の文脈",
  narratorContextNote: "文脈が広いほど、注意の控えに記憶を取られる。",
  narratorLayers: "画板に載せる語り手の層",
  narratorLayersNote: "−1 はすべて。語り手が処理装置で動く間は効かない。",
  keepFrameWeightsInRam: "絵の重みを主記憶に留める",

  keyHint: "OpenRouter の鍵は{state}。無ければ雲の段は動かず、すべて手元の画板が計算する。",
  asIsHint: "「そのまま」は画板があれば画板、無ければ処理装置。下の各段は個別に移せる。",
  componentsUnreadable: "部品の一覧が読めない。",
  ready: "完了",
  requiredNote: " · これが無いと遊べない",
  recommendedNote: " · 推奨",
  optionalNote: " · 任意",
  download: "落とす",
  monitorDetach: "資源計を切り離す",
  monitorAttach: "見出しに戻す",
  power: "電力",
  process: "処理",
  item: "品物",
  turnRunning: "手番の最中……",
  prevPage: "前の頁",
  readSpread: "開いた見開きを読み上げる",
  nextPage: "次の頁",

  asrParakeet: "Parakeet——速い、すぐ動く",
  asrWhisper: "Whisper large-v3——正確、別途落とす必要あり",
  notSaved: "保存できませんでした：{error}",
  cpuSlowNote: "とても遅い：1 段落に約 1 分半、GPU なら数秒です。実測で毎秒 14 文字。",
  computesWord: "処理先",
  imageWorkerNote: "GPU メモリを節約しますが、計算するのは GPU のままです。どこで計算するかは上の「{frame}」段階で選びます。",
  hardwareOnlyNote: "音声の設定は「声」、場面画像の描画は「画像」にあります。ここに残るのはハードウェアだけです。",
  gbUnit: "GB",
  wattUnit: "W",
  resources: "リソース",
  downloadFailed: "ダウンロードできませんでした：{error}",
  missingRequiredCount: "必須の不足：{n}",
  optionalAvailableCount: "追加可能：{n}",
  downloadRequired: "必須をダウンロード — {size}",
  allRequiredPresent: "必須はすべて揃っています",
  resumableNote: "合計 {size} 不足しています。ダウンロードは再開可能：中断しても最初からではなく、止まった場所から続きます。",
  bookEmpty: "本はまだ白紙です。物語を始めましょう。",
  readAloud: "読み上げ",
  flipPages: "ページをめくる · {n}",
};

const BY_LANGUAGE: Record<Language, PanelText> = {
  ru: RU,
  en: EN,
  es: ES,
  fr: FR,
  de: DE,
  zh: ZH,
  ja: JA,
};

export function panelText(language: Language | undefined): PanelText {
  return (language && BY_LANGUAGE[language]) || RU;
}
