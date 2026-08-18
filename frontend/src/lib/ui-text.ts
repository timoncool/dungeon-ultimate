import type { Language } from "@/lib/types";

/// Подписи интерфейса на всех языках игры.
///
/// Рассказ, журнал и озвучка говорили на семи языках с самого начала, а кнопки оставались
/// русскими: интерфейс переехал из прошлой версии как есть. Здесь он догоняет остальное.
///
/// Ключи названы по смыслу, а не по месту: одна и та же подпись встречается и в панели, и в
/// подсказке, и дублировать её незачем.
export type UiText = {
  // Шапка и истории
  newStory: string;
  newStoryShort: string;
  deleteStory: string;
  storiesSection: string;
  everythingInCloud: string;
  onThisMachine: string;

  // Ход
  continueTurn: string;
  retryTurn: string;
  eraseTurn: string;
  stopTurn: string;
  send: string;
  actionMode: string;
  speechMode: string;
  storyMode: string;
  actionPlaceholder: string;
  speechPlaceholder: string;
  storyPlaceholder: string;
  bookMode: string;
  feedMode: string;
  attachReferences: string;
  voiceInput: string;
  speakAloud: string;
  edit: string;
  generate: string;
  showMore: string;

  // Герой и колонки
  level: string;
  armorClass: string;
  effects: string;
  inventory: string;
  inventoryEmpty: string;
  quests: string;
  questsEmpty: string;
  journal: string;
  achievements: string;
  achievementsEmpty: string;
  achievementsTotal: string;
  achievementsLegendary: string;
  rarityCommon: string;
  rarityRare: string;
  rarityLegendary: string;
  storyLabel: string;
  storyDeleted: string;

  // Характеристики
  statStr: string;
  statDex: string;
  statCon: string;
  statInt: string;
  statWis: string;
  statCha: string;

  // Разделы правого меню
  party: string;
  worldAndRules: string;
  images: string;
  voice: string;
  enginesAndCloud: string;
  modelOnDevice: string;
  whatToDownload: string;
  dataOnDisk: string;
  support: string;
  atHand: string;

  // Стадии хода
  stageLoadingModel: string;
  stageProse: string;
  stageTools: string;
  stageMechanics: string;
  stageFrame: string;
  stageVoice: string;
  stageActions: string;

  // Где что считается
  allOnMyCard: string;
  cloudPrefix: string;
  cardPrefix: string;
  stopSpeaking: string;

  // Карточка кадра и состояния панелей
  collapse: string;
  frameRequested: string;
  cloudStagesOf: string;
  everythingInPlace: string;

  // Плашка обновления
  updateTitle: string;
  updateHow: string;
  whatsNew: string;
  hideUpdate: string;

  // Гибель героя
  heroDeadTitle: string;
  heroDeadHint: string;
};

const RU: UiText = {
  newStory: "Новая история",
  newStoryShort: "Новая",
  deleteStory: "Удалить текущую историю",
  storiesSection: "Мои истории",
  everythingInCloud: "Всё в облаке",
  onThisMachine: "На этой машине",

  continueTurn: "Продолжить",
  retryTurn: "Повторить",
  eraseTurn: "Стереть",
  stopTurn: "Прервать",
  send: "Отправить",
  actionMode: "Действие",
  speechMode: "Речь",
  storyMode: "История",
  actionPlaceholder: "Что ты делаешь?",
  speechPlaceholder: "Что ты говоришь?",
  storyPlaceholder: "Напиши следующую часть истории…",
  bookMode: "Книга",
  feedMode: "Лента",
  attachReferences: "Прикрепить референсы",
  voiceInput: "Голосовой ввод",
  speakAloud: "Озвучить",
  edit: "Изменить",
  generate: "Сгенерировать",
  showMore: "Показать больше",

  level: "ур.",
  armorClass: "Класс защиты",
  effects: "Эффекты",
  inventory: "Инвентарь",
  inventoryEmpty: "Пусто — добыча появится в бою",
  quests: "Задания",
  questsEmpty: "Пока ни одного. Задания дают жители мира — выслушай того, кому нужна помощь.",
  journal: "Журнал",
  achievements: "Достижения",
  achievementsEmpty:
    "Пока пусто. Награды дают за поступок — выстоять против сильнейшего, пощадить побеждённого, пройти сцену без единого удара. Их не выпрашивают: они случаются.",
  achievementsTotal: "Всего",
  achievementsLegendary: "легендарных",
  rarityCommon: "Достижение",
  rarityRare: "Редкое достижение",
  rarityLegendary: "Легендарное достижение",
  storyLabel: "История",
  storyDeleted: "История удалена",

  statStr: "СИЛ",
  statDex: "ЛОВ",
  statCon: "ВЫН",
  statInt: "ИНТ",
  statWis: "МУД",
  statCha: "ХАР",

  party: "Партия",
  worldAndRules: "Мир и правила",
  images: "Картинки",
  voice: "Голос",
  enginesAndCloud: "Движки и облако",
  modelOnDevice: "Модель на устройстве",
  whatToDownload: "Что скачать",
  dataOnDisk: "Данные на диске",
  support: "Поддержка",
  atHand: "Под рукой",

  stageLoadingModel: "Модель едет на карту, это разово…",
  stageProse: "Рассказчик пишет…",
  stageTools: "Сверяюсь с состоянием игры…",
  stageMechanics: "Свожу последствия хода…",
  stageFrame: "Оператор выбирает кадр…",
  stageVoice: "Читаю вслух…",
  stageActions: "Подбираю варианты действий…",

  collapse: "Свернуть",
  frameRequested: "Запрошен инструмент изображений.",
  cloudStagesOf: "В облаке {n} из {total}",
  everythingInPlace: "Всё на месте",

  allOnMyCard: "Всё на своей карте",
  cloudPrefix: "Облако",
  cardPrefix: "карта",
  stopSpeaking: "Остановить озвучку",

  updateTitle: "Вышла версия {latest} — у тебя {current}",
  updateHow: "Игра обновится сама: скачает новую версию и поставит её с твоего согласия. Сохранения, модели и настройки остаются на месте.",
  whatsNew: "Что нового",
  hideUpdate: "Скрыть сообщение об обновлении",

  heroDeadTitle: "Герой погиб. История окончена.",
  heroDeadHint: "Роковой ход можно отменить кнопкой «Стереть» — или начать новую историю.",
};

const EN: UiText = {
  newStory: "New story",
  newStoryShort: "New",
  deleteStory: "Delete this story",
  storiesSection: "My stories",
  everythingInCloud: "All in the cloud",
  onThisMachine: "On this machine",

  continueTurn: "Continue",
  retryTurn: "Retry",
  eraseTurn: "Erase",
  stopTurn: "Stop",
  send: "Send",
  actionMode: "Action",
  speechMode: "Speech",
  storyMode: "Story",
  actionPlaceholder: "What do you do?",
  speechPlaceholder: "What do you say?",
  storyPlaceholder: "Write the next part of the story…",
  bookMode: "Book",
  feedMode: "Feed",
  attachReferences: "Attach references",
  voiceInput: "Voice input",
  speakAloud: "Read aloud",
  edit: "Edit",
  generate: "Generate",
  showMore: "Show more",

  level: "lvl",
  armorClass: "Armour class",
  effects: "Effects",
  inventory: "Inventory",
  inventoryEmpty: "Empty — loot shows up in a fight",
  quests: "Quests",
  questsEmpty: "None yet. Quests come from the people of this world — hear out the one who needs help.",
  journal: "Journal",
  achievements: "Achievements",
  achievementsEmpty:
    "Nothing yet. Awards are given for a deed — outlasting a stronger foe, sparing the defeated, clearing a scene without a blow. They are not asked for: they happen.",
  achievementsTotal: "Total",
  achievementsLegendary: "legendary",
  rarityCommon: "Achievement",
  rarityRare: "Rare achievement",
  rarityLegendary: "Legendary achievement",
  storyLabel: "Story",
  storyDeleted: "Story deleted",

  statStr: "STR",
  statDex: "DEX",
  statCon: "CON",
  statInt: "INT",
  statWis: "WIS",
  statCha: "CHA",

  party: "Party",
  worldAndRules: "World and rules",
  images: "Images",
  voice: "Voice",
  enginesAndCloud: "Engines and cloud",
  modelOnDevice: "Model on device",
  whatToDownload: "What to download",
  dataOnDisk: "Data on disk",
  support: "Support",
  atHand: "At hand",

  stageLoadingModel: "The model is moving onto the card, once…",
  stageProse: "The narrator is writing…",
  stageTools: "Checking the state of the game…",
  stageMechanics: "Settling the consequences of the turn…",
  stageFrame: "The camera is choosing a shot…",
  stageVoice: "Reading aloud…",
  stageActions: "Picking what you could do…",

  collapse: "Collapse",
  frameRequested: "The image tool was called.",
  cloudStagesOf: "{n} of {total} in the cloud",
  everythingInPlace: "Everything is in place",

  allOnMyCard: "All on my card",
  cloudPrefix: "Cloud",
  cardPrefix: "card",
  stopSpeaking: "Stop reading",

  updateTitle: "Version {latest} is out — you have {current}",
  updateHow: "The game updates itself: it downloads the new version and installs it once you agree. Saves, models and settings stay where they are.",
  whatsNew: "What's new",
  hideUpdate: "Hide the update notice",

  heroDeadTitle: "The hero is dead. The story is over.",
  heroDeadHint: "The fatal turn can be undone with «Erase» — or start a new story.",
};

const ES: UiText = {
  newStory: "Historia nueva",
  newStoryShort: "Nueva",
  deleteStory: "Borrar esta historia",
  storiesSection: "Mis historias",
  everythingInCloud: "Todo en la nube",
  onThisMachine: "En esta máquina",

  continueTurn: "Continuar",
  retryTurn: "Repetir",
  eraseTurn: "Borrar",
  stopTurn: "Detener",
  send: "Enviar",
  actionMode: "Acción",
  speechMode: "Habla",
  storyMode: "Historia",
  actionPlaceholder: "¿Qué haces?",
  speechPlaceholder: "¿Qué dices?",
  storyPlaceholder: "Escribe la siguiente parte de la historia…",
  bookMode: "Libro",
  feedMode: "Muro",
  attachReferences: "Adjuntar referencias",
  voiceInput: "Entrada de voz",
  speakAloud: "Leer en voz alta",
  edit: "Editar",
  generate: "Generar",
  showMore: "Ver más",

  level: "niv.",
  armorClass: "Clase de armadura",
  effects: "Efectos",
  inventory: "Inventario",
  inventoryEmpty: "Vacío: el botín aparece en combate",
  quests: "Misiones",
  questsEmpty: "Ninguna todavía. Las misiones las dan los habitantes del mundo: escucha a quien necesita ayuda.",
  journal: "Diario",
  achievements: "Logros",
  achievementsEmpty:
    "Aún nada. Los premios se dan por una hazaña: resistir a un rival más fuerte, perdonar al vencido, superar una escena sin un solo golpe. No se piden: ocurren.",
  achievementsTotal: "Total",
  achievementsLegendary: "legendarios",
  rarityCommon: "Logro",
  rarityRare: "Logro raro",
  rarityLegendary: "Logro legendario",
  storyLabel: "Historia",
  storyDeleted: "Historia borrada",

  statStr: "FUE",
  statDex: "DES",
  statCon: "CON",
  statInt: "INT",
  statWis: "SAB",
  statCha: "CAR",

  party: "Grupo",
  worldAndRules: "Mundo y reglas",
  images: "Imágenes",
  voice: "Voz",
  enginesAndCloud: "Motores y nube",
  modelOnDevice: "Modelo en el equipo",
  whatToDownload: "Qué descargar",
  dataOnDisk: "Datos en el disco",
  support: "Apoyo",
  atHand: "A mano",

  stageLoadingModel: "El modelo sube a la tarjeta, una sola vez…",
  stageProse: "El narrador escribe…",
  stageTools: "Consultando el estado del juego…",
  stageMechanics: "Resolviendo las consecuencias del turno…",
  stageFrame: "La cámara elige el plano…",
  stageVoice: "Leyendo en voz alta…",
  stageActions: "Buscando qué podrías hacer…",

  collapse: "Plegar",
  frameRequested: "Se ha pedido la herramienta de imagen.",
  cloudStagesOf: "{n} de {total} en la nube",
  everythingInPlace: "Todo está en su sitio",

  allOnMyCard: "Todo en mi tarjeta",
  cloudPrefix: "Nube",
  cardPrefix: "tarjeta",
  stopSpeaking: "Detener la lectura",

  updateTitle: "Ha salido la versión {latest}; tú tienes {current}",
  updateHow: "El juego se actualiza solo: descarga la versión nueva y la instala con tu permiso. Partidas, modelos y ajustes se quedan donde están.",
  whatsNew: "Novedades",
  hideUpdate: "Ocultar el aviso de actualización",

  heroDeadTitle: "El héroe ha muerto. La historia terminó.",
  heroDeadHint: "El turno fatal se deshace con «Borrar», o empieza una historia nueva.",
};

const FR: UiText = {
  newStory: "Nouvelle histoire",
  newStoryShort: "Nouvelle",
  deleteStory: "Supprimer cette histoire",
  storiesSection: "Mes histoires",
  everythingInCloud: "Tout dans le nuage",
  onThisMachine: "Sur cette machine",

  continueTurn: "Continuer",
  retryTurn: "Refaire",
  eraseTurn: "Effacer",
  stopTurn: "Arrêter",
  send: "Envoyer",
  actionMode: "Action",
  speechMode: "Parole",
  storyMode: "Récit",
  actionPlaceholder: "Que fais-tu ?",
  speechPlaceholder: "Que dis-tu ?",
  storyPlaceholder: "Écris la suite de l'histoire…",
  bookMode: "Livre",
  feedMode: "Fil",
  attachReferences: "Joindre des références",
  voiceInput: "Saisie vocale",
  speakAloud: "Lire à voix haute",
  edit: "Modifier",
  generate: "Générer",
  showMore: "Voir plus",

  level: "niv.",
  armorClass: "Classe d'armure",
  effects: "Effets",
  inventory: "Inventaire",
  inventoryEmpty: "Vide — le butin arrive au combat",
  quests: "Quêtes",
  questsEmpty: "Aucune pour l'instant. Les quêtes viennent des habitants du monde : écoute celui qui a besoin d'aide.",
  journal: "Journal",
  achievements: "Récompenses",
  achievementsEmpty:
    "Rien encore. Les récompenses saluent un exploit : tenir face à plus fort, épargner le vaincu, traverser une scène sans un coup. On ne les demande pas : elles arrivent.",
  achievementsTotal: "Total",
  achievementsLegendary: "légendaires",
  rarityCommon: "Récompense",
  rarityRare: "Récompense rare",
  rarityLegendary: "Récompense légendaire",
  storyLabel: "Histoire",
  storyDeleted: "Histoire supprimée",

  statStr: "FOR",
  statDex: "DEX",
  statCon: "CON",
  statInt: "INT",
  statWis: "SAG",
  statCha: "CHA",

  party: "Groupe",
  worldAndRules: "Monde et règles",
  images: "Images",
  voice: "Voix",
  enginesAndCloud: "Moteurs et nuage",
  modelOnDevice: "Modèle sur la machine",
  whatToDownload: "Que télécharger",
  dataOnDisk: "Données sur le disque",
  support: "Soutien",
  atHand: "Sous la main",

  stageLoadingModel: "Le modèle monte sur la carte, une seule fois…",
  stageProse: "Le narrateur écrit…",
  stageTools: "Je vérifie l'état du jeu…",
  stageMechanics: "Je règle les conséquences du tour…",
  stageFrame: "La caméra choisit le plan…",
  stageVoice: "Lecture à voix haute…",
  stageActions: "Je cherche ce que tu pourrais faire…",

  collapse: "Replier",
  frameRequested: "L'outil d'image a été demandé.",
  cloudStagesOf: "{n} sur {total} dans le nuage",
  everythingInPlace: "Tout est en place",

  allOnMyCard: "Tout sur ma carte",
  cloudPrefix: "Nuage",
  cardPrefix: "carte",
  stopSpeaking: "Arrêter la lecture",

  updateTitle: "La version {latest} est sortie — tu as {current}",
  updateHow: "Le jeu se met à jour tout seul : il télécharge la nouvelle version et l'installe avec ton accord. Sauvegardes, modèles et réglages restent en place.",
  whatsNew: "Quoi de neuf",
  hideUpdate: "Masquer l'avis de mise à jour",

  heroDeadTitle: "Le héros est mort. L'histoire est finie.",
  heroDeadHint: "Le tour fatal s'annule avec « Effacer » — ou commence une nouvelle histoire.",
};

const DE: UiText = {
  newStory: "Neue Geschichte",
  newStoryShort: "Neu",
  deleteStory: "Diese Geschichte löschen",
  storiesSection: "Meine Geschichten",
  everythingInCloud: "Alles in der Cloud",
  onThisMachine: "Auf diesem Rechner",

  continueTurn: "Weiter",
  retryTurn: "Wiederholen",
  eraseTurn: "Löschen",
  stopTurn: "Abbrechen",
  send: "Senden",
  actionMode: "Handlung",
  speechMode: "Rede",
  storyMode: "Erzählung",
  actionPlaceholder: "Was tust du?",
  speechPlaceholder: "Was sagst du?",
  storyPlaceholder: "Schreib den nächsten Teil der Geschichte…",
  bookMode: "Buch",
  feedMode: "Verlauf",
  attachReferences: "Referenzen anhängen",
  voiceInput: "Spracheingabe",
  speakAloud: "Vorlesen",
  edit: "Ändern",
  generate: "Erzeugen",
  showMore: "Mehr zeigen",

  level: "St.",
  armorClass: "Rüstungsklasse",
  effects: "Effekte",
  inventory: "Inventar",
  inventoryEmpty: "Leer — Beute gibt es im Kampf",
  quests: "Aufträge",
  questsEmpty: "Noch keine. Aufträge geben die Bewohner der Welt — hör dem zu, der Hilfe braucht.",
  journal: "Journal",
  achievements: "Auszeichnungen",
  achievementsEmpty:
    "Noch nichts. Auszeichnungen gelten einer Tat: einem Stärkeren standhalten, den Besiegten verschonen, eine Szene ohne einen Schlag bestehen. Man erbittet sie nicht — sie geschehen.",
  achievementsTotal: "Gesamt",
  achievementsLegendary: "legendär",
  rarityCommon: "Auszeichnung",
  rarityRare: "Seltene Auszeichnung",
  rarityLegendary: "Legendäre Auszeichnung",
  storyLabel: "Geschichte",
  storyDeleted: "Geschichte gelöscht",

  statStr: "STÄ",
  statDex: "GES",
  statCon: "KON",
  statInt: "INT",
  statWis: "WEI",
  statCha: "CHA",

  party: "Gruppe",
  worldAndRules: "Welt und Regeln",
  images: "Bilder",
  voice: "Stimme",
  enginesAndCloud: "Engines und Cloud",
  modelOnDevice: "Modell auf dem Gerät",
  whatToDownload: "Was herunterladen",
  dataOnDisk: "Daten auf der Platte",
  support: "Unterstützung",
  atHand: "Zur Hand",

  stageLoadingModel: "Das Modell zieht auf die Karte, einmalig…",
  stageProse: "Der Erzähler schreibt…",
  stageTools: "Ich prüfe den Spielstand…",
  stageMechanics: "Ich verrechne die Folgen des Zuges…",
  stageFrame: "Die Kamera wählt die Einstellung…",
  stageVoice: "Ich lese vor…",
  stageActions: "Ich suche, was du tun könntest…",

  collapse: "Einklappen",
  frameRequested: "Das Bildwerkzeug wurde gerufen.",
  cloudStagesOf: "{n} von {total} in der Cloud",
  everythingInPlace: "Alles ist da",

  allOnMyCard: "Alles auf meiner Karte",
  cloudPrefix: "Cloud",
  cardPrefix: "Karte",
  stopSpeaking: "Vorlesen stoppen",

  updateTitle: "Version {latest} ist da — du hast {current}",
  updateHow: "Das Spiel aktualisiert sich selbst: Es lädt die neue Version und installiert sie mit deiner Zustimmung. Spielstände, Modelle und Einstellungen bleiben.",
  whatsNew: "Was ist neu",
  hideUpdate: "Update-Hinweis ausblenden",

  heroDeadTitle: "Der Held ist tot. Die Geschichte ist zu Ende.",
  heroDeadHint: "Der verhängnisvolle Zug lässt sich mit «Löschen» zurücknehmen — oder fang neu an.",
};

const ZH: UiText = {
  newStory: "新故事",
  newStoryShort: "新建",
  deleteStory: "删除这个故事",
  storiesSection: "我的故事",
  everythingInCloud: "全部在云端",
  onThisMachine: "在本机",

  continueTurn: "继续",
  retryTurn: "重来",
  eraseTurn: "抹去",
  stopTurn: "中断",
  send: "发送",
  actionMode: "行动",
  speechMode: "说话",
  storyMode: "叙述",
  actionPlaceholder: "你做什么？",
  speechPlaceholder: "你说什么？",
  storyPlaceholder: "写下故事的下一段……",
  bookMode: "书页",
  feedMode: "信息流",
  attachReferences: "附加参考图",
  voiceInput: "语音输入",
  speakAloud: "朗读",
  edit: "修改",
  generate: "生成",
  showMore: "显示更多",

  level: "等级",
  armorClass: "护甲等级",
  effects: "效果",
  inventory: "物品",
  inventoryEmpty: "空的——战斗中才有战利品",
  quests: "任务",
  questsEmpty: "还没有。任务来自这个世界的人——听听需要帮助的人怎么说。",
  journal: "日志",
  achievements: "成就",
  achievementsEmpty:
    "还没有。成就嘉奖的是壮举：挡住更强的对手、放过被击败者、未动一手化解场面。它不能索取，只会发生。",
  achievementsTotal: "共",
  achievementsLegendary: "传奇",
  rarityCommon: "成就",
  rarityRare: "稀有成就",
  rarityLegendary: "传奇成就",
  storyLabel: "故事",
  storyDeleted: "故事已删除",

  statStr: "力量",
  statDex: "敏捷",
  statCon: "体质",
  statInt: "智力",
  statWis: "感知",
  statCha: "魅力",

  party: "队伍",
  worldAndRules: "世界与规则",
  images: "画面",
  voice: "声音",
  enginesAndCloud: "引擎与云端",
  modelOnDevice: "本机模型",
  whatToDownload: "下载什么",
  dataOnDisk: "磁盘上的数据",
  support: "支持作者",
  atHand: "随手设置",

  stageLoadingModel: "模型正在装入显卡，只此一次……",
  stageProse: "叙述者正在书写……",
  stageTools: "正在核对游戏状态……",
  stageMechanics: "正在结算这一回合的后果……",
  stageFrame: "镜头正在选取画面……",
  stageVoice: "正在朗读……",
  stageActions: "正在想你能做什么……",

  collapse: "收起",
  frameRequested: "已调用绘图工具。",
  cloudStagesOf: "{total} 项中有 {n} 项在云端",
  everythingInPlace: "一切就绪",

  allOnMyCard: "全部在本机显卡",
  cloudPrefix: "云端",
  cardPrefix: "显卡",
  stopSpeaking: "停止朗读",

  updateTitle: "新版本 {latest} 已发布——你的是 {current}",
  updateHow: "游戏会自行更新：下载新版本，并在你同意后安装。存档、模型与设置都留在原处。",
  whatsNew: "有什么新变化",
  hideUpdate: "隐藏更新提示",

  heroDeadTitle: "主角已死，故事到此结束。",
  heroDeadHint: "致命的一回合可以用「抹去」撤销，或者开始新的故事。",
};

const JA: UiText = {
  newStory: "新しい物語",
  newStoryShort: "新規",
  deleteStory: "この物語を削除",
  storiesSection: "わたしの物語",
  everythingInCloud: "すべて雲の上",
  onThisMachine: "この機械の上",

  continueTurn: "続ける",
  retryTurn: "やり直す",
  eraseTurn: "消す",
  stopTurn: "中断",
  send: "送る",
  actionMode: "行動",
  speechMode: "台詞",
  storyMode: "地の文",
  actionPlaceholder: "どうする？",
  speechPlaceholder: "なんと言う？",
  storyPlaceholder: "物語の続きを書く……",
  bookMode: "本",
  feedMode: "流れ",
  attachReferences: "参考画像を添える",
  voiceInput: "音声入力",
  speakAloud: "読み上げ",
  edit: "編集",
  generate: "生成",
  showMore: "もっと見る",

  level: "Lv",
  armorClass: "防御値",
  effects: "効果",
  inventory: "持ち物",
  inventoryEmpty: "空っぽ——戦えば手に入る",
  quests: "依頼",
  questsEmpty: "まだない。依頼はこの世界の住人がくれる——助けを求める者の話を聞くこと。",
  journal: "日誌",
  achievements: "褒賞",
  achievementsEmpty:
    "まだ何もない。褒賞は行いに与えられる——格上に耐える、敗者を見逃す、一撃も交えず場を収める。求めるものではなく、起こるもの。",
  achievementsTotal: "全部で",
  achievementsLegendary: "伝説級",
  rarityCommon: "褒賞",
  rarityRare: "稀な褒賞",
  rarityLegendary: "伝説の褒賞",
  storyLabel: "物語",
  storyDeleted: "物語は削除済み",

  statStr: "筋力",
  statDex: "敏捷",
  statCon: "耐久",
  statInt: "知力",
  statWis: "判断",
  statCha: "魅力",

  party: "一行",
  worldAndRules: "世界と規則",
  images: "絵",
  voice: "声",
  enginesAndCloud: "エンジンと雲",
  modelOnDevice: "手元のモデル",
  whatToDownload: "何を落とすか",
  dataOnDisk: "ディスク上のデータ",
  support: "支援",
  atHand: "手元の設定",

  stageLoadingModel: "モデルを画板に載せている、これ一度きり……",
  stageProse: "語り手が書いている……",
  stageTools: "盤面を確かめている……",
  stageMechanics: "この手番の結果をまとめている……",
  stageFrame: "カメラが構図を選んでいる……",
  stageVoice: "読み上げている……",
  stageActions: "できることを探している……",

  collapse: "たたむ",
  frameRequested: "画像の道具が呼ばれた。",
  cloudStagesOf: "{total} のうち {n} が雲の上",
  everythingInPlace: "すべて揃っている",

  allOnMyCard: "すべて手元の画板で",
  cloudPrefix: "雲",
  cardPrefix: "画板",
  stopSpeaking: "読み上げを止める",

  updateTitle: "版 {latest} が出た——手元は {current}",
  updateHow: "ゲームは自分で更新する。新しい版を落とし、承諾のうえで入れる。セーブ・モデル・設定はそのまま。",
  whatsNew: "何が変わったか",
  hideUpdate: "更新の知らせを隠す",

  heroDeadTitle: "主人公は死んだ。物語はここで終わる。",
  heroDeadHint: "致命の手番は「消す」で取り消せる——あるいは新しい物語を始める。",
};

const BY_LANGUAGE: Record<Language, UiText> = {
  ru: RU,
  en: EN,
  es: ES,
  fr: FR,
  de: DE,
  zh: ZH,
  ja: JA,
};

/// Подписи на языке игры. Незнакомый язык — русский: он полон и был здесь первым.
export function uiText(language: Language | undefined): UiText {
  return (language && BY_LANGUAGE[language]) || RU;
}
