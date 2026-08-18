import type { Language } from "@/lib/types";

/// Подписи настроек, диалогов и сообщений: третий и последний словарь интерфейса.
///
/// Разделены по месту не ради красоты: главный экран — короткие кнопки, панели — пояснения,
/// здесь — формы и жанры. Один файл на всё превращался бы в простыню.
export type SettingsText = {
  // Диалог новой истории
  everyStoryStartsWithALine: string;
  startNewStory: string;
  pickSetting: string;
  whatIsThisStory: string;
  whoAreYou: string;
  name: string;
  optional: string;
  characterGender: string;
  genderMale: string;
  genderFemale: string;
  genderUnset: string;
  beginning: string;
  narratorSetsTheScene: string;
  writeTheOpeningYourself: string;
  openingHint: string;
  openingHintPlaceholder: string;
  yourOpeningPassage: string;
  cancel: string;
  beginStory: string;
  ownWorld: string;
  describeYourStart: string;
  fillForMe: string;
  thinkingUp: string;
  couldNotInvent: string;
  close: string;

  // Жанры
  genreFantasy: string;
  genreFantasyNote: string;
  genreFantasyWorld: string;
  genreFantasyRole: string;
  genreDetective: string;
  genreDetectiveNote: string;
  genreDetectiveWorld: string;
  genreDetectiveRole: string;
  genreCyberpunk: string;
  genreCyberpunkNote: string;
  genreCyberpunkWorld: string;
  genreCyberpunkRole: string;
  genrePostapoc: string;
  genrePostapocNote: string;
  genrePostapocWorld: string;
  genrePostapocRole: string;
  genreHorror: string;
  genreHorrorNote: string;
  genreHorrorWorld: string;
  genreHorrorRole: string;
  genreRomance: string;
  genreRomanceNote: string;
  genreRomanceWorld: string;
  genreRomanceRole: string;

  // Персонажи
  hero: string;
  details: string;
  skills: string;
  spells: string;
  thisCharacter: string;
  savedCharactersAppearHere: string;
  deleteThisCharacter: string;
  delete: string;
  clear: string;
  detailsPlaceholder: string;
  inventoryPlaceholder: string;
  skillsPlaceholder: string;
  spellsPlaceholder: string;
  fillCharacterForMe: string;
  removeDraftPortrait: string;
  regenerateAvatar: string;
  voiceAutoNote: string;
  enableMultiVoiceNote: string;

  // Мир и правила
  world: string;
  style: string;
  dndMode: string;
  randomEvents: string;
  dice: string;
  dice3d: string;
  diceSound: string;
  howTheStoryGoes: string;
  avoidRepeats: string;
  meaningfulEnding: string;
  companion: string;
  narratorPrompt: string;
  narratorPromptNote: string;
  fontSize: string;
  responseLength: string;
  lengthShort: string;
  lengthMedium: string;
  lengthLong: string;
  lengthEpic: string;
  language: string;

  // Картинки
  imageGeneration: string;
  models: string;
  size: string;
  ratio: string;
  autoImages: string;
  frameStyle: string;
  frameStyleNote: string;
  frameStylePlaceholder: string;
  imagePrompt: string;
  imagePromptNote: string;
  square: string;
  portrait: string;
  landscape: string;
  stepsPerFrame: string;
  stepsNote: string;
  promptStrength: string;
  promptStrengthNote: string;
  frameEngineReady: string;
  frameWeightsMissing: string;
  frameEngineNote: string;
  frameWeightsHint: string;
  rendering: string;

  // Голос
  autoNarration: string;
  narrationVoice: string;
  narrationOn: string;
  narrationOnNote: string;
  narrationParallel: string;
  narrationParallelNote: string;
  volume: string;
  speechRate: string;
  referenceSeconds: string;
  referenceSecondsNote: string;
  uploadOwnVoice: string;
  loading: string;
  synthesis: string;
  perCharacterVoices: string;
  stopRecording: string;

  // Инвентарь и задания
  slotWeapon: string;
  slotArmor: string;
  slotShield: string;
  slotTrinket: string;
  slotConsumable: string;
  slotMisc: string;
  unequip: string;
  equip: string;
  equippedHint: string;
  equipHint: string;
  loot: string;
  takeQuest: string;
  declineQuest: string;
  abandonQuest: string;
  dead: string;

  // Данные и поддержка
  wipeEverything: string;
  deleteAll: string;
  deleteThisStory: string;
  copyAddress: string;
  allWays: string;
  byCard: string;

  // Кнопки хода и подсказки
  openStoryTools: string;
  closeStoryTools: string;
  storyTools: string;
  tools: string;
  hideCharacter: string;
  showCharacter: string;
  hideMenu: string;
  showMenu: string;
  continueWithoutMe: string;
  regenerateLastPassage: string;
  removeLastExchange: string;
  heroDeadNoOne: string;
  save: string;

  // Остатки: демонстрационные предметы, подсказки полей, провайдер
  demoBlade: string;
  demoRing: string;
  demoPotion: string;
  demoCheck: string;
  detailsKeepPlaceholder: string;
  inventoryFullPlaceholder: string;
  skillsFullPlaceholder: string;
  spellsFullPlaceholder: string;
  thisComputer: string;
  provider: string;
  localServer: string;
  pickModel: string;
  orTypeIdByHand: string;
  onlyIfServerNeeds: string;
  images: string;
  storyWord: string;
  inventItem: string;
  audioBlocked: string;

  idea: string;
  stageNarrator: string;
  stageFrameWord: string;
  stageVoiceWord: string;
  stageSpeechWord: string;

  // Сообщения
  errorChat: string;
  errorItem: string;
  errorQuest: string;
  errorDelete: string;
  errorWipe: string;
  errorLibrary: string;
  errorSettings: string;
  errorImage: string;
  errorCharacterSave: string;
  errorCharacterUpdate: string;
  errorCharacterPortrait: string;
  errorCharacterDelete: string;
  errorGenerate: string;
  errorStoryStream: string;
  errorStoryRequest: string;
  errorStoryTimeout: string;
  errorStoryCreate: string;
  errorSaveChanges: string;
  errorVoiceLoad: string;
  errorTtsServer: string;
  errorCharacterGenerate: string;
  errorTextServer: string;
  errorImageTool: string;
  drawingFrame: string;
  formingPassage: string;
  generatingScene: string;
  imageServerRequested: string;
  imageServerFailed: string;
  modelFolderOpened: string;
  modelFolderFailed: string;
};

/// Тексты. Порядок ключей везде одинаков — так проще искать пропущенное.
const RU: SettingsText = {
  everyStoryStartsWithALine: "Каждая история начинается с одной строки.",
  startNewStory: "Начать новую историю",
  pickSetting: "Выбери сеттинг, скажи кто ты, и выбери как начнётся история.",
  whatIsThisStory: "О чём эта история?",
  whoAreYou: "Кто ты?",
  name: "Имя",
  optional: "(необязательно)",
  characterGender: "Пол персонажа",
  genderMale: "Мужской",
  genderFemale: "Женский",
  genderUnset: "Не указан",
  beginning: "Начало",
  narratorSetsTheScene: "Рассказчик задаёт сцену",
  writeTheOpeningYourself: "Написать начало самому",
  openingHint: "Подсказка начала",
  openingHintPlaceholder: "напр. начни с моего пробуждения в камере без памяти о прошлой ночи",
  yourOpeningPassage: "Твой вводный отрывок",
  cancel: "Отмена",
  beginStory: "Начать историю",
  ownWorld: "Свой мир",
  describeYourStart: "Опиши своё начало",
  fillForMe: "Заполнить за меня",
  thinkingUp: "Придумываю…",
  couldNotInvent: "Не получилось придумать — попробуй ещё раз.",
  close: "Закрыть",

  genreFantasy: "Фэнтези",
  genreFantasyNote: "Рыцари, магия, старые дороги",
  genreFantasyWorld:
    "Высокое фэнтези: враждующие королевства, древняя магия и дороги, что перестают быть безопасными после заката.",
  genreFantasyRole: "странствующий наёмник",
  genreDetective: "Детектив",
  genreDetectiveNote: "Дождь, секреты, незакрытые нити",
  genreDetectiveWorld:
    "Залитый дождём город, полный секретов, где каждое дело — дверь, которую кто-то хочет держать закрытой.",
  genreDetectiveRole: "частный детектив",
  genreCyberpunk: "Киберпанк",
  genreCyberpunkNote: "Неон, хром, дурные долги",
  genreCyberpunkWorld:
    "Залитый неоном мегаполис под властью корпораций, где память — валюта, и каждый кому-то должен.",
  genreCyberpunkRole: "выгоревший нетраннер",
  genrePostapoc: "Постапокалипсис",
  genrePostapocNote: "После конца всего",
  genrePostapocWorld:
    "Спустя годы после краха разрозненные выжившие шарят по руинам, меняются и травят байки о том, как было раньше.",
  genrePostapocRole: "сборщик хлама с картой",
  genreHorror: "Хоррор",
  genreHorrorNote: "Здесь что-то не так",
  genreHorrorWorld:
    "Глухой городок, где ночи тянутся долго, а местные не говорят о том, что в них творится.",
  genreHorrorRole: "приезжий",
  genreRomance: "Романтика",
  genreRomanceNote: "Искры в неожиданных местах",
  genreRomanceWorld:
    "Тесный приморский городок на исходе лета, где случайные встречи имеют свойство перерастать в нечто большее.",
  genreRomanceRole: "новичок с прошлым",

  hero: "Герой",
  details: "Детали",
  skills: "Навыки",
  spells: "Заклинания",
  thisCharacter: "Этот персонаж",
  savedCharactersAppearHere: "Сохранённые персонажи появятся здесь.",
  deleteThisCharacter: "Удалить этого персонажа?",
  delete: "Удалить",
  clear: "Очистить",
  detailsPlaceholder: "Короткие чёрные волосы, пацанка, сухой юмор...",
  inventoryPlaceholder: "Железный кинжал, фонарь, 12 серебра...",
  skillsPlaceholder: "Взлом замков, травничество...",
  spellsPlaceholder: "Починка, рука мага...",
  fillCharacterForMe: "Заполнить персонажа за меня",
  removeDraftPortrait: "Убрать черновой портрет",
  regenerateAvatar: "Перегенерировать аватар",
  voiceAutoNote: "Авто — стабильный отдельный голос по персонажу. Действует при «Разные голоса персонажей».",
  enableMultiVoiceNote: "Включи «Разные голоса персонажей», чтобы реплики читались этим голосом.",

  world: "Мир",
  style: "Стиль",
  dndMode: "⚔️ Режим D&D (статы, кубик, журнал)",
  randomEvents: "✨ Случайные события (баффы/проклятья)",
  dice: "🎲 Кубики",
  dice3d: "3D-бросок",
  diceSound: "Звук броска",
  howTheStoryGoes: "✨ Как ведётся рассказ",
  avoidRepeats: "Избегать повторов сцен",
  meaningfulEnding: "Осмысленная концовка",
  companion: "Спутник-комментатор",
  narratorPrompt: "Промпт рассказчика",
  narratorPromptNote: "Системный промпт рассказчика. Пусто — встроенный по умолчанию.",
  fontSize: "Размер шрифта",
  responseLength: "Длина ответа",
  lengthShort: "Кратко",
  lengthMedium: "Средне",
  lengthLong: "Длинно",
  lengthEpic: "Эпик",
  language: "Язык",

  imageGeneration: "Генерация изображений",
  models: "Модели",
  size: "Размер",
  ratio: "Соотношение",
  autoImages: "Авто-картинки",
  frameStyle: "Стиль кадра",
  frameStyleNote: "Приписывается к каждому кадру — так все картинки истории держат одну манеру.",
  frameStylePlaceholder: "напр. dark fantasy, масло, приглушённая палитра",
  imagePrompt: "Промпт изображений",
  imagePromptNote:
    "Инструкция модели по генерации изображений (FLUX-промпт остаётся на английском). Пусто — встроенный по умолчанию.",
  square: "Квадрат",
  portrait: "Портрет",
  landscape: "Пейзаж",
  stepsPerFrame: "Шагов на кадр",
  stepsNote: "Меньше шагов — быстрее кадр, но грубее. Krea-2 Turbo дистиллирована под восемь.",
  promptStrength: "Сила промпта",
  promptStrengthNote: "У дистиллированной модели должна оставаться единицей.",
  frameEngineReady: "Движок кадров готов",
  frameWeightsMissing: "Весов кадра нет",
  frameEngineNote: "Кадр рисуется на карте, память освобождается сразу после.",
  frameWeightsHint: "Скачайте веса — или переключите кадр в облако в разделе «Движки и облако».",
  rendering: "Отрисовка",

  autoNarration: "Автоозвучка",
  narrationVoice: "Голос озвучки",
  narrationOn: "Озвучка включена",
  narrationOnNote: "Выключенная озвучка не тратит ни карту, ни облако.",
  narrationParallel: "Озвучивать параллельно с ходом",
  narrationParallelNote: "Быстрее, но на карте это лишние гигабайты памяти — на слабой лучше выключить.",
  volume: "Громкость",
  speechRate: "Скорость речи",
  referenceSeconds: "Секунд эталона голоса",
  referenceSecondsNote: "Сколько секунд образца отдавать движку клонирования.",
  uploadOwnVoice: "Загрузить свой голос (.mp3)",
  loading: "Загрузка…",
  synthesis: "Синтез",
  perCharacterVoices: "Реплики персонажей — своими голосами",
  stopRecording: "Остановить запись",

  slotWeapon: "оружие",
  slotArmor: "броня",
  slotShield: "щит",
  slotTrinket: "украшение",
  slotConsumable: "расходник",
  slotMisc: "прочее",
  unequip: "Снять",
  equip: "Надеть",
  equippedHint: " · надето (нажмите чтобы снять)",
  equipHint: " · надеть",
  loot: "Добыча — ",
  takeQuest: "Взять",
  declineQuest: "Отказаться",
  abandonQuest: "Бросить",
  dead: "☠️ погиб",

  wipeEverything: "Полностью очистить приложение?",
  deleteAll: "Удалить всё",
  deleteThisStory: "Удалить эту историю?",
  copyAddress: "Скопировать адрес",
  allWays: "Все способы",
  byCard: "💳 Картой",

  openStoryTools: "Открыть инструменты истории",
  closeStoryTools: "Закрыть инструменты истории",
  storyTools: "Инструменты истории",
  tools: "Инструменты",
  hideCharacter: "Скрыть персонажа",
  showCharacter: "Показать персонажа",
  hideMenu: "Скрыть меню",
  showMenu: "Показать меню",
  continueWithoutMe: "Пусть рассказчик продолжит без тебя",
  regenerateLastPassage: "Перегенерировать последний отрывок",
  removeLastExchange: "Убрать последний обмен",
  heroDeadNoOne: "Герой погиб — писать больше некому",
  save: "Сохранить",

  demoBlade: "Клинок ярости",
  demoRing: "Кольцо стража",
  demoPotion: "Зелье лечения",
  demoCheck: "Ловкость · d20 = проверка",
  detailsKeepPlaceholder: "Детали, которые рассказчик должен сохранить...",
  inventoryFullPlaceholder: "Предметы, снаряжение, деньги, квестовые объекты...",
  skillsFullPlaceholder: "Таланты, умения, классовые особенности...",
  spellsFullPlaceholder: "Подготовленные заклинания, способности, заметки о перезарядке...",
  thisComputer: "Этот компьютер",
  provider: "Провайдер",
  localServer: "Локальный сервер",
  pickModel: "— выбери модель —",
  orTypeIdByHand: "или впиши id вручную",
  onlyIfServerNeeds: "только если сервер этого требует",
  images: "Изображения",
  storyWord: "История",
  inventItem: "Придумать за меня",
  audioBlocked: "Звук заблокирован окном: щёлкните по странице — озвучка пойдёт дальше.",

  idea: "Идея",
  stageNarrator: "рассказчик",
  stageFrameWord: "кадр",
  stageVoiceWord: "голос",
  stageSpeechWord: "речь",

  errorChat: "Не удалось загрузить чат.",
  errorItem: "Не удалось обновить предмет.",
  errorQuest: "Не удалось изменить задание.",
  errorDelete: "Не удалось удалить.",
  errorWipe: "Не удалось очистить локальные данные.",
  errorLibrary: "Не удалось загрузить библиотеку историй.",
  errorSettings: "Не удалось сохранить настройки.",
  errorImage: "Не удалось загрузить изображение.",
  errorCharacterSave: "Не удалось сохранить персонажа.",
  errorCharacterUpdate: "Не удалось обновить персонажа.",
  errorCharacterPortrait: "Не удалось загрузить портрет персонажа.",
  errorCharacterDelete: "Не удалось удалить персонажа.",
  errorGenerate: "Не удалось сгенерировать изображение.",
  errorStoryStream: "Поток истории прервался.",
  errorStoryRequest: "Не удалось выполнить запрос истории.",
  errorStoryTimeout:
    "Рассказчик слишком долго отвечал. Модель ещё может работать в фоне; подожди немного, затем повтори или начни заново.",
  errorStoryCreate: "Не удалось создать историю.",
  errorSaveChanges: "Не удалось сохранить изменения.",
  errorVoiceLoad: "Не удалось загрузить голос.",
  errorTtsServer: "Сервер озвучки не запущен (порт 8081).",
  errorCharacterGenerate: "Не удалось сгенерировать персонажа.",
  errorTextServer: "Не удалось — запущен ли текстовый сервер?",
  errorImageTool: "Ошибка инструмента изображений.",
  drawingFrame: "Рисуется кадр сцены…",
  formingPassage: "Формируется следующий отрывок…",
  generatingScene: "Генерирую сцену...",
  imageServerRequested: "Запрошен запуск сервера изображений.",
  imageServerFailed: "Не удалось запустить сервер изображений.",
  modelFolderOpened: "Папка модели открыта.",
  modelFolderFailed: "Не удалось открыть папку модели.",
};

const EN: SettingsText = {
  everyStoryStartsWithALine: "Every story begins with a single line.",
  startNewStory: "Start a new story",
  pickSetting: "Pick a setting, say who you are, and choose how the story begins.",
  whatIsThisStory: "What is this story about?",
  whoAreYou: "Who are you?",
  name: "Name",
  optional: "(optional)",
  characterGender: "Character's gender",
  genderMale: "Male",
  genderFemale: "Female",
  genderUnset: "Unspecified",
  beginning: "Beginning",
  narratorSetsTheScene: "The narrator sets the scene",
  writeTheOpeningYourself: "Write the opening yourself",
  openingHint: "Opening hint",
  openingHintPlaceholder: "e.g. start with me waking in a cell with no memory of last night",
  yourOpeningPassage: "Your opening passage",
  cancel: "Cancel",
  beginStory: "Begin the story",
  ownWorld: "My own world",
  describeYourStart: "Describe your own start",
  fillForMe: "Fill it in for me",
  thinkingUp: "Thinking…",
  couldNotInvent: "Could not come up with it — try again.",
  close: "Close",

  genreFantasy: "Fantasy",
  genreFantasyNote: "Knights, magic, old roads",
  genreFantasyWorld:
    "High fantasy: warring kingdoms, ancient magic and roads that stop being safe after dusk.",
  genreFantasyRole: "a wandering sellsword",
  genreDetective: "Detective",
  genreDetectiveNote: "Rain, secrets, loose threads",
  genreDetectiveWorld:
    "A rain-soaked city full of secrets, where every case is a door somebody wants kept shut.",
  genreDetectiveRole: "a private detective",
  genreCyberpunk: "Cyberpunk",
  genreCyberpunkNote: "Neon, chrome, bad debts",
  genreCyberpunkWorld:
    "A neon-drowned megacity run by corporations, where memory is currency and everyone owes someone.",
  genreCyberpunkRole: "a burned-out netrunner",
  genrePostapoc: "Post-apocalypse",
  genrePostapocNote: "After the end of everything",
  genrePostapocWorld:
    "Years after the collapse, scattered survivors pick through ruins, trade, and tell tales of how it used to be.",
  genrePostapocRole: "a scrapper with a map",
  genreHorror: "Horror",
  genreHorrorNote: "Something here is wrong",
  genreHorrorWorld:
    "A backwater town where the nights run long and the locals do not talk about what happens in them.",
  genreHorrorRole: "a newcomer",
  genreRomance: "Romance",
  genreRomanceNote: "Sparks in unexpected places",
  genreRomanceWorld:
    "A cramped seaside town at the end of summer, where chance meetings tend to grow into something more.",
  genreRomanceRole: "a newcomer with a past",

  hero: "Hero",
  details: "Details",
  skills: "Skills",
  spells: "Spells",
  thisCharacter: "This character",
  savedCharactersAppearHere: "Saved characters will show up here.",
  deleteThisCharacter: "Delete this character?",
  delete: "Delete",
  clear: "Clear",
  detailsPlaceholder: "Short black hair, tomboy, dry humour...",
  inventoryPlaceholder: "Iron dagger, lantern, 12 silver...",
  skillsPlaceholder: "Lockpicking, herbalism...",
  spellsPlaceholder: "Mending, mage hand...",
  fillCharacterForMe: "Fill the character in for me",
  removeDraftPortrait: "Remove the draft portrait",
  regenerateAvatar: "Regenerate the avatar",
  voiceAutoNote: "Auto — a stable separate voice per character. Works with «Per-character voices».",
  enableMultiVoiceNote: "Turn on «Per-character voices» so lines are read in this voice.",

  world: "World",
  style: "Style",
  dndMode: "⚔️ D&D mode (stats, dice, journal)",
  randomEvents: "✨ Random events (blessings/curses)",
  dice: "🎲 Dice",
  dice3d: "3D roll",
  diceSound: "Roll sound",
  howTheStoryGoes: "✨ How the story is told",
  avoidRepeats: "Avoid repeating scenes",
  meaningfulEnding: "A meaningful ending",
  companion: "Companion commentator",
  narratorPrompt: "Narrator prompt",
  narratorPromptNote: "The narrator's system prompt. Blank means the built-in default.",
  fontSize: "Font size",
  responseLength: "Response length",
  lengthShort: "Short",
  lengthMedium: "Medium",
  lengthLong: "Long",
  lengthEpic: "Epic",
  language: "Language",

  imageGeneration: "Image generation",
  models: "Models",
  size: "Size",
  ratio: "Ratio",
  autoImages: "Auto images",
  frameStyle: "Frame style",
  frameStyleNote: "Prepended to every frame, so all the art of a story keeps one manner.",
  frameStylePlaceholder: "e.g. dark fantasy, oil paint, muted palette",
  imagePrompt: "Image prompt",
  imagePromptNote:
    "The model's instruction for generating images (the FLUX prompt stays English). Blank means the built-in default.",
  square: "Square",
  portrait: "Portrait",
  landscape: "Landscape",
  stepsPerFrame: "Steps per frame",
  stepsNote: "Fewer steps means a faster but rougher frame. Krea-2 Turbo is distilled for eight.",
  promptStrength: "Prompt strength",
  promptStrengthNote: "For a distilled model it should stay at one.",
  frameEngineReady: "The frame engine is ready",
  frameWeightsMissing: "The frame weights are missing",
  frameEngineNote: "The frame is drawn on the card; memory is freed right after.",
  frameWeightsHint: "Download the weights — or move the frame to the cloud in «Engines and cloud».",
  rendering: "Rendering",

  autoNarration: "Auto narration",
  narrationVoice: "Narration voice",
  narrationOn: "Narration is on",
  narrationOnNote: "Narration that is off costs neither the card nor the cloud.",
  narrationParallel: "Narrate in parallel with the turn",
  narrationParallelNote: "Faster, but on the card it costs extra gigabytes — turn it off on a weak one.",
  volume: "Volume",
  speechRate: "Speech rate",
  referenceSeconds: "Seconds of the voice sample",
  referenceSecondsNote: "How many seconds of the sample to hand to the cloning engine.",
  uploadOwnVoice: "Upload your own voice (.mp3)",
  loading: "Loading…",
  synthesis: "Synthesis",
  perCharacterVoices: "Character lines — in their own voices",
  stopRecording: "Stop recording",

  slotWeapon: "weapon",
  slotArmor: "armour",
  slotShield: "shield",
  slotTrinket: "trinket",
  slotConsumable: "consumable",
  slotMisc: "misc",
  unequip: "Unequip",
  equip: "Equip",
  equippedHint: " · equipped (click to take off)",
  equipHint: " · equip",
  loot: "Loot — ",
  takeQuest: "Take",
  declineQuest: "Decline",
  abandonQuest: "Abandon",
  dead: "☠️ dead",

  wipeEverything: "Wipe the app completely?",
  deleteAll: "Delete everything",
  deleteThisStory: "Delete this story?",
  copyAddress: "Copy the address",
  allWays: "All the ways",
  byCard: "💳 By card",

  openStoryTools: "Open the story tools",
  closeStoryTools: "Close the story tools",
  storyTools: "Story tools",
  tools: "Tools",
  hideCharacter: "Hide the character",
  showCharacter: "Show the character",
  hideMenu: "Hide the menu",
  showMenu: "Show the menu",
  continueWithoutMe: "Let the narrator carry on without you",
  regenerateLastPassage: "Regenerate the last passage",
  removeLastExchange: "Remove the last exchange",
  heroDeadNoOne: "The hero is dead — there is no one left to write",
  save: "Save",

  demoBlade: "Blade of fury",
  demoRing: "Warden's ring",
  demoPotion: "Healing potion",
  demoCheck: "Dexterity · d20 = check",
  detailsKeepPlaceholder: "Details the narrator should keep...",
  inventoryFullPlaceholder: "Items, gear, money, quest objects...",
  skillsFullPlaceholder: "Talents, skills, class features...",
  spellsFullPlaceholder: "Prepared spells, abilities, notes on recharge...",
  thisComputer: "This computer",
  provider: "Provider",
  localServer: "Local server",
  pickModel: "— pick a model —",
  orTypeIdByHand: "or type the id by hand",
  onlyIfServerNeeds: "only if the server asks for it",
  images: "Images",
  storyWord: "Story",
  inventItem: "Invent it for me",
  audioBlocked: "The window blocked the sound: click the page and narration will carry on.",

  idea: "Idea",
  stageNarrator: "narrator",
  stageFrameWord: "frame",
  stageVoiceWord: "voice",
  stageSpeechWord: "speech",

  errorChat: "Could not load the chat.",
  errorItem: "Could not update the item.",
  errorQuest: "Could not change the quest.",
  errorDelete: "Could not delete.",
  errorWipe: "Could not clear the local data.",
  errorLibrary: "Could not load the story library.",
  errorSettings: "Could not save the settings.",
  errorImage: "Could not load the image.",
  errorCharacterSave: "Could not save the character.",
  errorCharacterUpdate: "Could not update the character.",
  errorCharacterPortrait: "Could not load the character portrait.",
  errorCharacterDelete: "Could not delete the character.",
  errorGenerate: "Could not generate the image.",
  errorStoryStream: "The story stream broke off.",
  errorStoryRequest: "Could not carry out the story request.",
  errorStoryTimeout:
    "The narrator took too long. The model may still be working in the background; wait a little, then retry or start over.",
  errorStoryCreate: "Could not create the story.",
  errorSaveChanges: "Could not save the changes.",
  errorVoiceLoad: "Could not load the voice.",
  errorTtsServer: "The narration server is not running (port 8081).",
  errorCharacterGenerate: "Could not generate the character.",
  errorTextServer: "Failed — is the text server running?",
  errorImageTool: "The image tool failed.",
  drawingFrame: "Drawing the scene…",
  formingPassage: "Forming the next passage…",
  generatingScene: "Generating the scene...",
  imageServerRequested: "The image server was asked to start.",
  imageServerFailed: "Could not start the image server.",
  modelFolderOpened: "The model folder is open.",
  modelFolderFailed: "Could not open the model folder.",
};

const ES: SettingsText = {
  everyStoryStartsWithALine: "Toda historia empieza con una sola línea.",
  startNewStory: "Empezar una historia nueva",
  pickSetting: "Elige un escenario, di quién eres y cómo quieres que empiece la historia.",
  whatIsThisStory: "¿De qué trata esta historia?",
  whoAreYou: "¿Quién eres?",
  name: "Nombre",
  optional: "(opcional)",
  characterGender: "Género del personaje",
  genderMale: "Masculino",
  genderFemale: "Femenino",
  genderUnset: "Sin especificar",
  beginning: "Comienzo",
  narratorSetsTheScene: "El narrador plantea la escena",
  writeTheOpeningYourself: "Escribir el comienzo por tu cuenta",
  openingHint: "Pista para el comienzo",
  openingHintPlaceholder: "p. ej. empieza conmigo despertando en una celda sin recordar la noche anterior",
  yourOpeningPassage: "Tu párrafo inicial",
  cancel: "Cancelar",
  beginStory: "Empezar la historia",
  ownWorld: "Mi propio mundo",
  describeYourStart: "Describe tu propio comienzo",
  fillForMe: "Rellenarlo por mí",
  thinkingUp: "Pensando…",
  couldNotInvent: "No se pudo idear nada — inténtalo de nuevo.",
  close: "Cerrar",

  genreFantasy: "Fantasía",
  genreFantasyNote: "Caballeros, magia, viejos caminos",
  genreFantasyWorld:
    "Alta fantasía: reinos en guerra, magia ancestral y caminos que dejan de ser seguros después del anochecer.",
  genreFantasyRole: "un mercenario errante",
  genreDetective: "Detective",
  genreDetectiveNote: "Lluvia, secretos, cabos sueltos",
  genreDetectiveWorld:
    "Una ciudad empapada de lluvia y llena de secretos, donde cada caso es una puerta que alguien quiere mantener cerrada.",
  genreDetectiveRole: "un detective privado",
  genreCyberpunk: "Ciberpunk",
  genreCyberpunkNote: "Neón, cromo, deudas turbias",
  genreCyberpunkWorld:
    "Una megaciudad anegada en neón y gobernada por corporaciones, donde la memoria es moneda de cambio y todos le deben algo a alguien.",
  genreCyberpunkRole: "un netrunner quemado",
  genrePostapoc: "Posapocalíptico",
  genrePostapocNote: "Después del fin de todo",
  genrePostapocWorld:
    "Años después del colapso, supervivientes dispersos rebuscan entre las ruinas, comercian y cuentan historias de cómo era todo antes.",
  genrePostapocRole: "un chatarrero con un mapa",
  genreHorror: "Terror",
  genreHorrorNote: "Aquí algo no anda bien",
  genreHorrorWorld:
    "Un pueblo perdido donde las noches se alargan y los vecinos no hablan de lo que ocurre en ellas.",
  genreHorrorRole: "un recién llegado",
  genreRomance: "Romance",
  genreRomanceNote: "Chispas donde menos lo esperas",
  genreRomanceWorld:
    "Un pueblo costero pequeño al final del verano, donde los encuentros casuales suelen convertirse en algo más.",
  genreRomanceRole: "un recién llegado con un pasado",

  hero: "Héroe",
  details: "Detalles",
  skills: "Habilidades",
  spells: "Hechizos",
  thisCharacter: "Este personaje",
  savedCharactersAppearHere: "Los personajes guardados aparecerán aquí.",
  deleteThisCharacter: "¿Eliminar este personaje?",
  delete: "Eliminar",
  clear: "Borrar",
  detailsPlaceholder: "Pelo negro corto, un poco marimacho, humor seco...",
  inventoryPlaceholder: "Daga de hierro, linterna, 12 monedas de plata...",
  skillsPlaceholder: "Forzar cerraduras, herbolaria...",
  spellsPlaceholder: "Remendar, mano de mago...",
  fillCharacterForMe: "Rellenar el personaje por mí",
  removeDraftPortrait: "Quitar el retrato provisional",
  regenerateAvatar: "Regenerar el avatar",
  voiceAutoNote: "Auto — una voz propia y estable para cada personaje. Funciona junto con «Voces por personaje».",
  enableMultiVoiceNote: "Activa «Voces por personaje» para que las frases se lean con esta voz.",

  world: "Mundo",
  style: "Estilo",
  dndMode: "⚔️ Modo D&D (estadísticas, dado, diario)",
  randomEvents: "✨ Eventos aleatorios (bendiciones/maldiciones)",
  dice: "🎲 Dados",
  dice3d: "Tirada en 3D",
  diceSound: "Sonido de la tirada",
  howTheStoryGoes: "✨ Cómo se cuenta la historia",
  avoidRepeats: "Evitar escenas repetidas",
  meaningfulEnding: "Un final con sentido",
  companion: "Compañero comentarista",
  narratorPrompt: "Prompt del narrador",
  narratorPromptNote: "El prompt de sistema del narrador. Si lo dejas vacío, se usa el predeterminado.",
  fontSize: "Tamaño de letra",
  responseLength: "Longitud de la respuesta",
  lengthShort: "Corta",
  lengthMedium: "Media",
  lengthLong: "Larga",
  lengthEpic: "Épica",
  language: "Idioma",

  imageGeneration: "Generación de imágenes",
  models: "Modelos",
  size: "Tamaño",
  ratio: "Proporción",
  autoImages: "Imágenes automáticas",
  frameStyle: "Estilo de imagen",
  frameStyleNote: "Se añade a cada imagen, así todas las imágenes de la historia mantienen un mismo estilo.",
  frameStylePlaceholder: "p. ej. dark fantasy, óleo, paleta apagada",
  imagePrompt: "Prompt de imágenes",
  imagePromptNote:
    "La instrucción del modelo para generar imágenes (el prompt de FLUX se mantiene en inglés). Si la dejas vacía, se usa la predeterminada.",
  square: "Cuadrado",
  portrait: "Vertical",
  landscape: "Horizontal",
  stepsPerFrame: "Pasos por imagen",
  stepsNote: "Menos pasos: imagen más rápida pero más tosca. Krea-2 Turbo está destilada para ocho.",
  promptStrength: "Fuerza del prompt",
  promptStrengthNote: "En un modelo destilado, debe mantenerse en uno.",
  frameEngineReady: "El motor de imágenes está listo",
  frameWeightsMissing: "Faltan los pesos del motor de imágenes",
  frameEngineNote: "La imagen se genera en tu tarjeta gráfica y la memoria se libera justo después.",
  frameWeightsHint: "Descarga los pesos — o cambia la generación de imágenes a la nube en «Motores y nube».",
  rendering: "Renderizando",

  autoNarration: "Narración automática",
  narrationVoice: "Voz de la narración",
  narrationOn: "Narración activada",
  narrationOnNote: "Con la narración apagada no se gasta ni tarjeta gráfica ni nube.",
  narrationParallel: "Narrar en paralelo al turno",
  narrationParallelNote: "Más rápido, pero en tu tarjeta gráfica consume gigabytes extra — desactívalo si es modesta.",
  volume: "Volumen",
  speechRate: "Velocidad de habla",
  referenceSeconds: "Segundos de muestra de voz",
  referenceSecondsNote: "Cuántos segundos de la muestra se le pasan al motor de clonación.",
  uploadOwnVoice: "Subir tu propia voz (.mp3)",
  loading: "Cargando…",
  synthesis: "Síntesis",
  perCharacterVoices: "Frases de personajes — con sus propias voces",
  stopRecording: "Detener grabación",

  slotWeapon: "arma",
  slotArmor: "armadura",
  slotShield: "escudo",
  slotTrinket: "amuleto",
  slotConsumable: "consumible",
  slotMisc: "otros",
  unequip: "Quitar",
  equip: "Equipar",
  equippedHint: " · equipado (haz clic para quitarlo)",
  equipHint: " · equipar",
  loot: "Botín — ",
  takeQuest: "Aceptar",
  declineQuest: "Rechazar",
  abandonQuest: "Abandonar",
  dead: "☠️ muerto",

  wipeEverything: "¿Borrar la app por completo?",
  deleteAll: "Eliminar todo",
  deleteThisStory: "¿Eliminar esta historia?",
  copyAddress: "Copiar la dirección",
  allWays: "Todas las formas",
  byCard: "💳 Con tarjeta",

  openStoryTools: "Abrir las herramientas de la historia",
  closeStoryTools: "Cerrar las herramientas de la historia",
  storyTools: "Herramientas de la historia",
  tools: "Herramientas",
  hideCharacter: "Ocultar el personaje",
  showCharacter: "Mostrar el personaje",
  hideMenu: "Ocultar el menú",
  showMenu: "Mostrar el menú",
  continueWithoutMe: "Dejar que el narrador continúe sin ti",
  regenerateLastPassage: "Regenerar el último párrafo",
  removeLastExchange: "Quitar el último intercambio",
  heroDeadNoOne: "El héroe ha muerto — ya no queda quién escriba",
  save: "Guardar",

  demoBlade: "Espada de la furia",
  demoRing: "Anillo del guardián",
  demoPotion: "Poción de curación",
  demoCheck: "Destreza · d20 = prueba",
  detailsKeepPlaceholder: "Detalles que el narrador debe recordar...",
  inventoryFullPlaceholder: "Objetos, equipo, dinero, objetos de misión...",
  skillsFullPlaceholder: "Talentos, habilidades, rasgos de clase...",
  spellsFullPlaceholder: "Hechizos preparados, habilidades, notas sobre recarga...",
  thisComputer: "Este equipo",
  provider: "Proveedor",
  localServer: "Servidor local",
  pickModel: "— elige un modelo —",
  orTypeIdByHand: "o escribe el id a mano",
  onlyIfServerNeeds: "solo si el servidor lo pide",
  images: "Imágenes",
  storyWord: "Historia",
  inventItem: "Inventarlo por mí",
  audioBlocked: "La ventana bloqueó el sonido: haz clic en la página y la narración continuará.",

  idea: "Idea",
  stageNarrator: "narrador",
  stageFrameWord: "imagen",
  stageVoiceWord: "voz",
  stageSpeechWord: "habla",

  errorChat: "No se pudo cargar el chat.",
  errorItem: "No se pudo actualizar el objeto.",
  errorQuest: "No se pudo cambiar la misión.",
  errorDelete: "No se pudo eliminar.",
  errorWipe: "No se pudieron borrar los datos locales.",
  errorLibrary: "No se pudo cargar la biblioteca de historias.",
  errorSettings: "No se pudo guardar la configuración.",
  errorImage: "No se pudo cargar la imagen.",
  errorCharacterSave: "No se pudo guardar el personaje.",
  errorCharacterUpdate: "No se pudo actualizar el personaje.",
  errorCharacterPortrait: "No se pudo cargar el retrato del personaje.",
  errorCharacterDelete: "No se pudo eliminar el personaje.",
  errorGenerate: "No se pudo generar la imagen.",
  errorStoryStream: "El flujo de la historia se interrumpió.",
  errorStoryRequest: "No se pudo completar la solicitud de la historia.",
  errorStoryTimeout:
    "El narrador tardó demasiado en responder. Puede que el modelo siga trabajando en segundo plano; espera un poco y luego reinténtalo o empieza de nuevo.",
  errorStoryCreate: "No se pudo crear la historia.",
  errorSaveChanges: "No se pudieron guardar los cambios.",
  errorVoiceLoad: "No se pudo cargar la voz.",
  errorTtsServer: "El servidor de narración no está en marcha (puerto 8081).",
  errorCharacterGenerate: "No se pudo generar el personaje.",
  errorTextServer: "Ha fallado — ¿está en marcha el servidor de texto?",
  errorImageTool: "Error en la herramienta de imágenes.",
  drawingFrame: "Dibujando la escena…",
  formingPassage: "Formando el siguiente párrafo…",
  generatingScene: "Generando la escena...",
  imageServerRequested: "Se ha pedido iniciar el servidor de imágenes.",
  imageServerFailed: "No se pudo iniciar el servidor de imágenes.",
  modelFolderOpened: "La carpeta del modelo está abierta.",
  modelFolderFailed: "No se pudo abrir la carpeta del modelo.",
};

const FR: SettingsText = {
  everyStoryStartsWithALine: "Chaque histoire commence par une seule ligne.",
  startNewStory: "Commencer une nouvelle histoire",
  pickSetting: "Choisis un univers, dis qui tu es, et choisis comment l'histoire commence.",
  whatIsThisStory: "De quoi parle cette histoire ?",
  whoAreYou: "Qui es-tu ?",
  name: "Nom",
  optional: "(facultatif)",
  characterGender: "Genre du personnage",
  genderMale: "Masculin",
  genderFemale: "Féminin",
  genderUnset: "Non précisé",
  beginning: "Début",
  narratorSetsTheScene: "Le narrateur plante le décor",
  writeTheOpeningYourself: "Écrire le début toi-même",
  openingHint: "Indice de départ",
  openingHintPlaceholder: "ex. commence avec mon réveil dans une cellule, sans souvenir de la nuit dernière",
  yourOpeningPassage: "Ton passage d'ouverture",
  cancel: "Annuler",
  beginStory: "Commencer l'histoire",
  ownWorld: "Mon propre monde",
  describeYourStart: "Décris ton propre départ",
  fillForMe: "Remplir à ma place",
  thinkingUp: "Réflexion…",
  couldNotInvent: "Impossible d'inventer quelque chose — réessaie.",
  close: "Fermer",

  genreFantasy: "Fantasy",
  genreFantasyNote: "Chevaliers, magie, vieilles routes",
  genreFantasyWorld:
    "Fantasy épique : royaumes en guerre, magie ancienne et routes qui cessent d'être sûres après le crépuscule.",
  genreFantasyRole: "un mercenaire errant",
  genreDetective: "Polar",
  genreDetectiveNote: "Pluie, secrets, fils non résolus",
  genreDetectiveWorld:
    "Une ville noyée sous la pluie et pleine de secrets, où chaque affaire est une porte que quelqu'un veut garder fermée.",
  genreDetectiveRole: "un détective privé",
  genreCyberpunk: "Cyberpunk",
  genreCyberpunkNote: "Néons, chrome, mauvaises dettes",
  genreCyberpunkWorld:
    "Une mégapole noyée de néons dirigée par les corporations, où la mémoire est une monnaie et où tout le monde doit quelque chose à quelqu'un.",
  genreCyberpunkRole: "un netrunner grillé",
  genrePostapoc: "Post-apocalyptique",
  genrePostapocNote: "Après la fin de tout",
  genrePostapocWorld:
    "Des années après l'effondrement, des survivants éparpillés fouillent les ruines, échangent et racontent comment c'était avant.",
  genrePostapocRole: "un ferrailleur avec une carte",
  genreHorror: "Horreur",
  genreHorrorNote: "Quelque chose ne va pas ici",
  genreHorrorWorld:
    "Une bourgade isolée où les nuits n'en finissent pas et où les habitants ne parlent pas de ce qui s'y passe.",
  genreHorrorRole: "un nouveau venu",
  genreRomance: "Romance",
  genreRomanceNote: "Des étincelles là où on ne les attend pas",
  genreRomanceWorld:
    "Une petite ville côtière à la fin de l'été, où les rencontres du hasard ont tendance à devenir bien plus.",
  genreRomanceRole: "un nouveau venu avec un passé",

  hero: "Héros",
  details: "Détails",
  skills: "Compétences",
  spells: "Sorts",
  thisCharacter: "Ce personnage",
  savedCharactersAppearHere: "Les personnages enregistrés apparaîtront ici.",
  deleteThisCharacter: "Supprimer ce personnage ?",
  delete: "Supprimer",
  clear: "Effacer",
  detailsPlaceholder: "Cheveux courts et noirs, garçon manqué, humour pince-sans-rire…",
  inventoryPlaceholder: "Dague en fer, lanterne, 12 pièces d'argent…",
  skillsPlaceholder: "Crochetage, herboristerie…",
  spellsPlaceholder: "Réparation, main du mage…",
  fillCharacterForMe: "Remplir le personnage à ma place",
  removeDraftPortrait: "Retirer le portrait provisoire",
  regenerateAvatar: "Régénérer l'avatar",
  voiceAutoNote: "Auto — une voix distincte et stable par personnage. S'applique avec « Voix par personnage ».",
  enableMultiVoiceNote: "Active « Voix par personnage » pour que les répliques soient lues avec cette voix.",

  world: "Monde",
  style: "Style",
  dndMode: "⚔️ Mode D&D (statistiques, dé, journal)",
  randomEvents: "✨ Événements aléatoires (bénédictions/malédictions)",
  dice: "🎲 Dés",
  dice3d: "Lancer en 3D",
  diceSound: "Son du lancer",
  howTheStoryGoes: "✨ Comment l'histoire est racontée",
  avoidRepeats: "Éviter de répéter les scènes",
  meaningfulEnding: "Une fin qui a du sens",
  companion: "Compagnon commentateur",
  narratorPrompt: "Prompt du narrateur",
  narratorPromptNote: "Le prompt système du narrateur. Vide = valeur par défaut intégrée.",
  fontSize: "Taille de police",
  responseLength: "Longueur de la réponse",
  lengthShort: "Courte",
  lengthMedium: "Moyenne",
  lengthLong: "Longue",
  lengthEpic: "Épique",
  language: "Langue",

  imageGeneration: "Génération d'images",
  models: "Modèles",
  size: "Taille",
  ratio: "Format",
  autoImages: "Images automatiques",
  frameStyle: "Style des images",
  frameStyleNote: "Ajouté devant chaque image, pour que tous les visuels de l'histoire gardent la même patte.",
  frameStylePlaceholder: "ex. dark fantasy, peinture à l'huile, palette sourde",
  imagePrompt: "Prompt des images",
  imagePromptNote:
    "L'instruction donnée au modèle de génération d'images (le prompt FLUX reste en anglais). Vide = valeur par défaut intégrée.",
  square: "Carré",
  portrait: "Portrait",
  landscape: "Paysage",
  stepsPerFrame: "Étapes par image",
  stepsNote: "Moins d'étapes donne une image plus rapide mais plus grossière. Krea-2 Turbo est distillé pour huit.",
  promptStrength: "Force du prompt",
  promptStrengthNote: "Pour un modèle distillé, elle doit rester à un.",
  frameEngineReady: "Le moteur d'images est prêt",
  frameWeightsMissing: "Les poids de l'image sont manquants",
  frameEngineNote: "L'image est générée sur la carte graphique ; la mémoire est libérée aussitôt après.",
  frameWeightsHint: "Télécharge les poids — ou bascule les images vers le cloud dans « Moteurs et cloud ».",
  rendering: "Rendu",

  autoNarration: "Narration automatique",
  narrationVoice: "Voix de narration",
  narrationOn: "Narration activée",
  narrationOnNote: "Une narration désactivée ne consomme ni la carte graphique ni le cloud.",
  narrationParallel: "Narrer en parallèle du tour",
  narrationParallelNote:
    "Plus rapide, mais cela coûte des gigaoctets de mémoire supplémentaires sur la carte graphique — désactive-le sur une carte peu puissante.",
  volume: "Volume",
  speechRate: "Débit de parole",
  referenceSeconds: "Secondes de référence vocale",
  referenceSecondsNote: "Combien de secondes de l'échantillon donner au moteur de clonage.",
  uploadOwnVoice: "Importer ta propre voix (.mp3)",
  loading: "Chargement…",
  synthesis: "Synthèse",
  perCharacterVoices: "Répliques des personnages — avec leurs propres voix",
  stopRecording: "Arrêter l'enregistrement",

  slotWeapon: "arme",
  slotArmor: "armure",
  slotShield: "bouclier",
  slotTrinket: "bijou",
  slotConsumable: "consommable",
  slotMisc: "divers",
  unequip: "Retirer",
  equip: "Équiper",
  equippedHint: " · équipé (cliquer pour retirer)",
  equipHint: " · équiper",
  loot: "Butin — ",
  takeQuest: "Accepter",
  declineQuest: "Refuser",
  abandonQuest: "Abandonner",
  dead: "☠️ mort",

  wipeEverything: "Effacer complètement l'application ?",
  deleteAll: "Tout supprimer",
  deleteThisStory: "Supprimer cette histoire ?",
  copyAddress: "Copier l'adresse",
  allWays: "Tous les moyens",
  byCard: "💳 Par carte",

  openStoryTools: "Ouvrir les outils de l'histoire",
  closeStoryTools: "Fermer les outils de l'histoire",
  storyTools: "Outils de l'histoire",
  tools: "Outils",
  hideCharacter: "Masquer le personnage",
  showCharacter: "Afficher le personnage",
  hideMenu: "Masquer le menu",
  showMenu: "Afficher le menu",
  continueWithoutMe: "Laisser le narrateur continuer sans toi",
  regenerateLastPassage: "Régénérer le dernier passage",
  removeLastExchange: "Supprimer le dernier échange",
  heroDeadNoOne: "Le héros est mort — il n'y a plus personne pour écrire",
  save: "Enregistrer",

  demoBlade: "Lame de la fureur",
  demoRing: "Anneau du gardien",
  demoPotion: "Potion de soin",
  demoCheck: "Dextérité · d20 = jet",
  detailsKeepPlaceholder: "Détails que le narrateur doit retenir…",
  inventoryFullPlaceholder: "Objets, équipement, argent, objets de quête…",
  skillsFullPlaceholder: "Talents, compétences, particularités de classe…",
  spellsFullPlaceholder: "Sorts préparés, capacités, notes de récupération…",
  thisComputer: "Cet ordinateur",
  provider: "Fournisseur",
  localServer: "Serveur local",
  pickModel: "— choisir un modèle —",
  orTypeIdByHand: "ou saisis l'id à la main",
  onlyIfServerNeeds: "seulement si le serveur le demande",
  images: "Images",
  storyWord: "Histoire",
  inventItem: "Inventer à ma place",
  audioBlocked: "La fenêtre a bloqué le son : clique sur la page et la narration reprendra.",

  idea: "Idée",
  stageNarrator: "narrateur",
  stageFrameWord: "image",
  stageVoiceWord: "voix",
  stageSpeechWord: "parole",

  errorChat: "Impossible de charger le chat.",
  errorItem: "Impossible de mettre à jour l'objet.",
  errorQuest: "Impossible de modifier la quête.",
  errorDelete: "Impossible de supprimer.",
  errorWipe: "Impossible d'effacer les données locales.",
  errorLibrary: "Impossible de charger la bibliothèque d'histoires.",
  errorSettings: "Impossible d'enregistrer les paramètres.",
  errorImage: "Impossible de charger l'image.",
  errorCharacterSave: "Impossible d'enregistrer le personnage.",
  errorCharacterUpdate: "Impossible de mettre à jour le personnage.",
  errorCharacterPortrait: "Impossible de charger le portrait du personnage.",
  errorCharacterDelete: "Impossible de supprimer le personnage.",
  errorGenerate: "Impossible de générer l'image.",
  errorStoryStream: "Le flux de l'histoire s'est interrompu.",
  errorStoryRequest: "Impossible d'exécuter la requête de l'histoire.",
  errorStoryTimeout:
    "Le narrateur a mis trop de temps à répondre. Le modèle travaille peut-être encore en arrière-plan ; attends un peu, puis réessaie ou recommence.",
  errorStoryCreate: "Impossible de créer l'histoire.",
  errorSaveChanges: "Impossible d'enregistrer les modifications.",
  errorVoiceLoad: "Impossible de charger la voix.",
  errorTtsServer: "Le serveur de narration n'est pas lancé (port 8081).",
  errorCharacterGenerate: "Impossible de générer le personnage.",
  errorTextServer: "Échec — le serveur de texte est-il lancé ?",
  errorImageTool: "Erreur de l'outil d'images.",
  drawingFrame: "Dessin de l'image de la scène…",
  formingPassage: "Formation du prochain passage…",
  generatingScene: "Génération de la scène…",
  imageServerRequested: "Démarrage du serveur d'images demandé.",
  imageServerFailed: "Impossible de démarrer le serveur d'images.",
  modelFolderOpened: "Le dossier du modèle est ouvert.",
  modelFolderFailed: "Impossible d'ouvrir le dossier du modèle.",
};

const DE: SettingsText = {
  everyStoryStartsWithALine: "Jede Geschichte beginnt mit einer einzigen Zeile.",
  startNewStory: "Neue Geschichte beginnen",
  pickSetting: "Wähle ein Setting, sag, wer du bist, und entscheide, wie die Geschichte beginnt.",
  whatIsThisStory: "Wovon handelt diese Geschichte?",
  whoAreYou: "Wer bist du?",
  name: "Name",
  optional: "(optional)",
  characterGender: "Geschlecht des Charakters",
  genderMale: "Männlich",
  genderFemale: "Weiblich",
  genderUnset: "Nicht angegeben",
  beginning: "Anfang",
  narratorSetsTheScene: "Der Erzähler setzt die Szene",
  writeTheOpeningYourself: "Den Anfang selbst schreiben",
  openingHint: "Hinweis zum Anfang",
  openingHintPlaceholder: "z. B. beginne damit, dass ich in einer Zelle aufwache und mich an letzte Nacht nicht erinnere",
  yourOpeningPassage: "Dein Eröffnungstext",
  cancel: "Abbrechen",
  beginStory: "Geschichte beginnen",
  ownWorld: "Meine eigene Welt",
  describeYourStart: "Beschreibe deinen eigenen Anfang",
  fillForMe: "Für mich ausfüllen",
  thinkingUp: "Denke nach…",
  couldNotInvent: "Ist mir nichts eingefallen — versuch es noch mal.",
  close: "Schließen",

  genreFantasy: "Fantasy",
  genreFantasyNote: "Ritter, Magie, alte Straßen",
  genreFantasyWorld:
    "High Fantasy: verfeindete Königreiche, uralte Magie und Straßen, die nach Einbruch der Dunkelheit nicht mehr sicher sind.",
  genreFantasyRole: "ein umherziehender Söldner",
  genreDetective: "Detektiv",
  genreDetectiveNote: "Regen, Geheimnisse, lose Fäden",
  genreDetectiveWorld:
    "Eine regennasse Stadt voller Geheimnisse, in der jeder Fall eine Tür ist, die jemand verschlossen halten will.",
  genreDetectiveRole: "ein Privatdetektiv",
  genreCyberpunk: "Cyberpunk",
  genreCyberpunkNote: "Neon, Chrom, faule Schulden",
  genreCyberpunkWorld:
    "Eine in Neonlicht ertrunkene Megacity in der Hand von Konzernen, wo Erinnerung Währung ist und jeder jemandem etwas schuldet.",
  genreCyberpunkRole: "ein ausgebrannter Netrunner",
  genrePostapoc: "Postapokalypse",
  genrePostapocNote: "Nach dem Ende von allem",
  genrePostapocWorld:
    "Jahre nach dem Zusammenbruch durchstöbern verstreute Überlebende Ruinen, handeln und erzählen Geschichten davon, wie es früher war.",
  genrePostapocRole: "ein Schrottsammler mit einer Karte",
  genreHorror: "Horror",
  genreHorrorNote: "Hier stimmt etwas nicht",
  genreHorrorWorld:
    "Ein Kaff, in dem die Nächte kein Ende nehmen und die Einheimischen nicht darüber reden, was in ihnen geschieht.",
  genreHorrorRole: "ein Neuankömmling",
  genreRomance: "Romantik",
  genreRomanceNote: "Funken an unerwarteten Orten",
  genreRomanceWorld:
    "Ein enges Küstenstädtchen am Ende des Sommers, wo aus zufälligen Begegnungen gern mehr wird.",
  genreRomanceRole: "ein Neuankömmling mit Vergangenheit",

  hero: "Held",
  details: "Details",
  skills: "Fähigkeiten",
  spells: "Zauber",
  thisCharacter: "Dieser Charakter",
  savedCharactersAppearHere: "Gespeicherte Charaktere erscheinen hier.",
  deleteThisCharacter: "Diesen Charakter löschen?",
  delete: "Löschen",
  clear: "Leeren",
  detailsPlaceholder: "Kurze schwarze Haare, burschikos, trockener Humor...",
  inventoryPlaceholder: "Eiserner Dolch, Laterne, 12 Silberstücke...",
  skillsPlaceholder: "Schlösserknacken, Kräuterkunde...",
  spellsPlaceholder: "Ausbessern, Magierhand...",
  fillCharacterForMe: "Charakter für mich ausfüllen",
  removeDraftPortrait: "Entwurfsporträt entfernen",
  regenerateAvatar: "Avatar neu generieren",
  voiceAutoNote: "Automatisch — eine feste eigene Stimme pro Charakter. Wirkt mit «Eigene Stimmen pro Charakter».",
  enableMultiVoiceNote: "Schalte «Eigene Stimmen pro Charakter» ein, damit die Zeilen mit dieser Stimme gelesen werden.",

  world: "Welt",
  style: "Stil",
  dndMode: "⚔️ D&D-Modus (Werte, Würfel, Journal)",
  randomEvents: "✨ Zufallsereignisse (Segen/Flüche)",
  dice: "🎲 Würfel",
  dice3d: "3D-Wurf",
  diceSound: "Würfelgeräusch",
  howTheStoryGoes: "✨ Wie die Geschichte erzählt wird",
  avoidRepeats: "Wiederholende Szenen vermeiden",
  meaningfulEnding: "Ein bedeutungsvolles Ende",
  companion: "Begleiter-Kommentator",
  narratorPrompt: "Erzähler-Prompt",
  narratorPromptNote: "Der System-Prompt des Erzählers. Leer bedeutet die eingebaute Standardeinstellung.",
  fontSize: "Schriftgröße",
  responseLength: "Antwortlänge",
  lengthShort: "Kurz",
  lengthMedium: "Mittel",
  lengthLong: "Lang",
  lengthEpic: "Episch",
  language: "Sprache",

  imageGeneration: "Bildgenerierung",
  models: "Modelle",
  size: "Größe",
  ratio: "Seitenverhältnis",
  autoImages: "Automatische Bilder",
  frameStyle: "Bildstil",
  frameStyleNote: "Wird jedem Bild vorangestellt, damit alle Bilder einer Geschichte einen Stil behalten.",
  frameStylePlaceholder: "z. B. dark fantasy, Ölgemälde, gedämpfte Farbpalette",
  imagePrompt: "Bild-Prompt",
  imagePromptNote:
    "Die Anweisung an das Modell zur Bildgenerierung (der FLUX-Prompt bleibt Englisch). Leer bedeutet die eingebaute Standardeinstellung.",
  square: "Quadrat",
  portrait: "Hochformat",
  landscape: "Querformat",
  stepsPerFrame: "Schritte pro Bild",
  stepsNote: "Weniger Schritte bedeutet ein schnelleres, aber gröberes Bild. Krea-2 Turbo ist auf acht destilliert.",
  promptStrength: "Prompt-Stärke",
  promptStrengthNote: "Bei einem destillierten Modell sollte sie bei eins bleiben.",
  frameEngineReady: "Die Bild-Engine ist bereit",
  frameWeightsMissing: "Die Bild-Gewichte fehlen",
  frameEngineNote: "Das Bild wird auf der Grafikkarte gezeichnet; der Speicher wird danach sofort freigegeben.",
  frameWeightsHint: "Lade die Gewichte herunter — oder verlagere das Bild in die Cloud unter «Engines und Cloud».",
  rendering: "Rendering",

  autoNarration: "Automatische Vertonung",
  narrationVoice: "Erzählstimme",
  narrationOn: "Vertonung ist an",
  narrationOnNote: "Ausgeschaltete Vertonung kostet weder die Grafikkarte noch die Cloud.",
  narrationParallel: "Parallel zum Zug vertonen",
  narrationParallelNote: "Schneller, kostet auf der Grafikkarte aber zusätzliche Gigabyte — bei einer schwachen Karte lieber ausschalten.",
  volume: "Lautstärke",
  speechRate: "Sprechgeschwindigkeit",
  referenceSeconds: "Sekunden der Stimmprobe",
  referenceSecondsNote: "Wie viele Sekunden der Probe an die Klon-Engine übergeben werden.",
  uploadOwnVoice: "Eigene Stimme hochladen (.mp3)",
  loading: "Lädt…",
  synthesis: "Synthese",
  perCharacterVoices: "Charakterzeilen — mit eigenen Stimmen",
  stopRecording: "Aufnahme stoppen",

  slotWeapon: "Waffe",
  slotArmor: "Rüstung",
  slotShield: "Schild",
  slotTrinket: "Schmuckstück",
  slotConsumable: "Verbrauchsgegenstand",
  slotMisc: "Sonstiges",
  unequip: "Ablegen",
  equip: "Anlegen",
  equippedHint: " · angelegt (klicken zum Ablegen)",
  equipHint: " · anlegen",
  loot: "Beute — ",
  takeQuest: "Annehmen",
  declineQuest: "Ablehnen",
  abandonQuest: "Aufgeben",
  dead: "☠️ tot",

  wipeEverything: "Die App komplett zurücksetzen?",
  deleteAll: "Alles löschen",
  deleteThisStory: "Diese Geschichte löschen?",
  copyAddress: "Adresse kopieren",
  allWays: "Alle Möglichkeiten",
  byCard: "💳 Mit Karte",

  openStoryTools: "Story-Werkzeuge öffnen",
  closeStoryTools: "Story-Werkzeuge schließen",
  storyTools: "Story-Werkzeuge",
  tools: "Werkzeuge",
  hideCharacter: "Charakter ausblenden",
  showCharacter: "Charakter anzeigen",
  hideMenu: "Menü ausblenden",
  showMenu: "Menü anzeigen",
  continueWithoutMe: "Den Erzähler ohne dich weitermachen lassen",
  regenerateLastPassage: "Letzten Abschnitt neu generieren",
  removeLastExchange: "Letzten Austausch entfernen",
  heroDeadNoOne: "Der Held ist tot — es gibt niemanden mehr, der weiterschreibt",
  save: "Speichern",

  demoBlade: "Klinge der Wut",
  demoRing: "Ring des Wächters",
  demoPotion: "Heiltrank",
  demoCheck: "Geschicklichkeit · d20 = Probe",
  detailsKeepPlaceholder: "Details, die sich der Erzähler merken soll...",
  inventoryFullPlaceholder: "Gegenstände, Ausrüstung, Geld, Questobjekte...",
  skillsFullPlaceholder: "Talente, Fertigkeiten, Klassenmerkmale...",
  spellsFullPlaceholder: "Vorbereitete Zauber, Fähigkeiten, Notizen zur Aufladung...",
  thisComputer: "Dieser Computer",
  provider: "Anbieter",
  localServer: "Lokaler Server",
  pickModel: "— Modell wählen —",
  orTypeIdByHand: "oder die ID von Hand eingeben",
  onlyIfServerNeeds: "nur wenn der Server danach fragt",
  images: "Bilder",
  storyWord: "Geschichte",
  inventItem: "Für mich ausdenken",
  audioBlocked: "Das Fenster hat den Ton blockiert: Klicke auf die Seite, dann läuft die Vertonung weiter.",

  idea: "Idee",
  stageNarrator: "Erzähler",
  stageFrameWord: "Bild",
  stageVoiceWord: "Stimme",
  stageSpeechWord: "Sprechen",

  errorChat: "Der Chat konnte nicht geladen werden.",
  errorItem: "Der Gegenstand konnte nicht aktualisiert werden.",
  errorQuest: "Die Quest konnte nicht geändert werden.",
  errorDelete: "Löschen fehlgeschlagen.",
  errorWipe: "Die lokalen Daten konnten nicht gelöscht werden.",
  errorLibrary: "Die Geschichtenbibliothek konnte nicht geladen werden.",
  errorSettings: "Die Einstellungen konnten nicht gespeichert werden.",
  errorImage: "Das Bild konnte nicht geladen werden.",
  errorCharacterSave: "Der Charakter konnte nicht gespeichert werden.",
  errorCharacterUpdate: "Der Charakter konnte nicht aktualisiert werden.",
  errorCharacterPortrait: "Das Charakterporträt konnte nicht geladen werden.",
  errorCharacterDelete: "Der Charakter konnte nicht gelöscht werden.",
  errorGenerate: "Das Bild konnte nicht generiert werden.",
  errorStoryStream: "Der Geschichten-Stream wurde unterbrochen.",
  errorStoryRequest: "Die Story-Anfrage konnte nicht ausgeführt werden.",
  errorStoryTimeout:
    "Der Erzähler hat zu lange gebraucht. Das Modell arbeitet vielleicht noch im Hintergrund; warte kurz und versuch es dann erneut oder fang neu an.",
  errorStoryCreate: "Die Geschichte konnte nicht erstellt werden.",
  errorSaveChanges: "Die Änderungen konnten nicht gespeichert werden.",
  errorVoiceLoad: "Die Stimme konnte nicht geladen werden.",
  errorTtsServer: "Der Vertonungsserver läuft nicht (Port 8081).",
  errorCharacterGenerate: "Der Charakter konnte nicht generiert werden.",
  errorTextServer: "Fehlgeschlagen — läuft der Textserver?",
  errorImageTool: "Das Bild-Tool ist fehlgeschlagen.",
  drawingFrame: "Die Szene wird gezeichnet…",
  formingPassage: "Der nächste Abschnitt entsteht…",
  generatingScene: "Szene wird generiert...",
  imageServerRequested: "Der Bildserver wurde zum Start aufgefordert.",
  imageServerFailed: "Der Bildserver konnte nicht gestartet werden.",
  modelFolderOpened: "Der Modellordner ist geöffnet.",
  modelFolderFailed: "Der Modellordner konnte nicht geöffnet werden.",
};

const ZH: SettingsText = {
  everyStoryStartsWithALine: "每个故事都始于一行文字。",
  startNewStory: "开始新故事",
  pickSetting: "选择一个背景设定，说说你是谁，再决定故事如何开始。",
  whatIsThisStory: "这是一个怎样的故事？",
  whoAreYou: "你是谁？",
  name: "姓名",
  optional: "（可选）",
  characterGender: "角色性别",
  genderMale: "男",
  genderFemale: "女",
  genderUnset: "未指定",
  beginning: "开篇",
  narratorSetsTheScene: "由讲述者铺陈场景",
  writeTheOpeningYourself: "自己写开篇",
  openingHint: "开篇提示",
  openingHintPlaceholder: "例如：从我在牢房中醒来、想不起昨晚发生了什么开始",
  yourOpeningPassage: "你的开篇段落",
  cancel: "取消",
  beginStory: "开始故事",
  ownWorld: "自定义世界",
  describeYourStart: "描述你的开局",
  fillForMe: "帮我填写",
  thinkingUp: "构思中…",
  couldNotInvent: "没能想出来——再试一次吧。",
  close: "关闭",

  genreFantasy: "奇幻",
  genreFantasyNote: "骑士、魔法、古老的道路",
  genreFantasyWorld:
    "高奇幻世界：交战的王国、古老的魔法，以及入夜后便不再安全的道路。",
  genreFantasyRole: "一名流浪佣兵",
  genreDetective: "侦探",
  genreDetectiveNote: "雨水、秘密、悬而未解的线索",
  genreDetectiveWorld:
    "一座被雨水浸透、充满秘密的城市，每一桩案子都是一扇有人想要关紧的门。",
  genreDetectiveRole: "一名私家侦探",
  genreCyberpunk: "赛博朋克",
  genreCyberpunkNote: "霓虹、铬合金、还不清的债",
  genreCyberpunkWorld:
    "一座被霓虹淹没、由企业统治的超级都市，记忆就是货币，人人都欠着谁的债。",
  genreCyberpunkRole: "一名心力交瘁的网络行者",
  genrePostapoc: "后启示录",
  genrePostapocNote: "万物终结之后",
  genrePostapocWorld:
    "崩溃发生多年之后，散落各处的幸存者在废墟中翻找，彼此交易，讲述从前的故事。",
  genrePostapocRole: "一名带着地图的拾荒者",
  genreHorror: "恐怖",
  genreHorrorNote: "这里有些不对劲",
  genreHorrorWorld:
    "一座偏僻小镇，夜晚格外漫长，当地人从不谈论夜里发生的事。",
  genreHorrorRole: "一名初来者",
  genreRomance: "爱情",
  genreRomanceNote: "意想不到之处擦出的火花",
  genreRomanceWorld:
    "夏末的一座狭小海滨小镇，偶然的相遇总会生出更多故事。",
  genreRomanceRole: "一名带着往事的初来者",

  hero: "主角",
  details: "细节",
  skills: "技能",
  spells: "法术",
  thisCharacter: "此角色",
  savedCharactersAppearHere: "已保存的角色会显示在这里。",
  deleteThisCharacter: "删除此角色？",
  delete: "删除",
  clear: "清空",
  detailsPlaceholder: "黑色短发，假小子性格，冷幽默……",
  inventoryPlaceholder: "铁匕首、提灯、12枚银币……",
  skillsPlaceholder: "开锁、草药学……",
  spellsPlaceholder: "修复术、法师之手……",
  fillCharacterForMe: "帮我填写角色",
  removeDraftPortrait: "移除草稿头像",
  regenerateAvatar: "重新生成头像",
  voiceAutoNote: "自动——为每个角色分配固定的独立音色。需配合「角色分音」使用。",
  enableMultiVoiceNote: "请打开「角色分音」，台词才会用这个音色朗读。",

  world: "世界",
  style: "风格",
  dndMode: "⚔️ D&D 模式（属性、骰子、日志）",
  randomEvents: "✨ 随机事件（祝福／诅咒）",
  dice: "🎲 骰子",
  dice3d: "3D 掷骰",
  diceSound: "掷骰音效",
  howTheStoryGoes: "✨ 故事的讲述方式",
  avoidRepeats: "避免场景重复",
  meaningfulEnding: "有意义的结局",
  companion: "旁白伙伴",
  narratorPrompt: "讲述者提示词",
  narratorPromptNote: "讲述者的系统提示词。留空则使用内置默认值。",
  fontSize: "字号",
  responseLength: "回复长度",
  lengthShort: "简短",
  lengthMedium: "适中",
  lengthLong: "较长",
  lengthEpic: "史诗",
  language: "语言",

  imageGeneration: "图像生成",
  models: "模型",
  size: "尺寸",
  ratio: "比例",
  autoImages: "自动配图",
  frameStyle: "画面风格",
  frameStyleNote: "会附加到每一帧画面前面，让整个故事的插图保持统一风格。",
  frameStylePlaceholder: "例如：暗黑奇幻、油画质感、低饱和配色",
  imagePrompt: "图像提示词",
  imagePromptNote:
    "用于指导模型生成图像的指令（FLUX 提示词保留英文）。留空则使用内置默认值。",
  square: "正方形",
  portrait: "竖版",
  landscape: "横版",
  stepsPerFrame: "每帧步数",
  stepsNote: "步数越少出图越快，但画面也越粗糙。Krea-2 Turbo 是为八步蒸馏优化的。",
  promptStrength: "提示词强度",
  promptStrengthNote: "对蒸馏模型而言，该值应保持为一。",
  frameEngineReady: "出图引擎已就绪",
  frameWeightsMissing: "缺少出图模型权重",
  frameEngineNote: "画面在显卡上绘制，绘制完成后会立即释放显存。",
  frameWeightsHint: "请下载权重文件——或在「引擎与云端」中把出图切换到云端。",
  rendering: "渲染中",

  autoNarration: "自动朗读",
  narrationVoice: "朗读音色",
  narrationOn: "朗读已开启",
  narrationOnNote: "关闭朗读既不占用显卡，也不消耗云端资源。",
  narrationParallel: "与回合同步朗读",
  narrationParallelNote: "速度更快，但会在显卡上额外占用几个 GB 显存——性能较弱时建议关闭。",
  volume: "音量",
  speechRate: "语速",
  referenceSeconds: "语音样本时长（秒）",
  referenceSecondsNote: "交给克隆引擎的样本时长，单位为秒。",
  uploadOwnVoice: "上传自己的声音（.mp3）",
  loading: "加载中…",
  synthesis: "合成",
  perCharacterVoices: "角色台词——各用各的音色",
  stopRecording: "停止录音",

  slotWeapon: "武器",
  slotArmor: "护甲",
  slotShield: "盾牌",
  slotTrinket: "饰品",
  slotConsumable: "消耗品",
  slotMisc: "杂物",
  unequip: "卸下",
  equip: "装备",
  equippedHint: " · 已装备（点击卸下）",
  equipHint: " · 装备",
  loot: "战利品——",
  takeQuest: "接受",
  declineQuest: "拒绝",
  abandonQuest: "放弃",
  dead: "☠️ 已死亡",

  wipeEverything: "彻底清空应用？",
  deleteAll: "删除全部",
  deleteThisStory: "删除这个故事？",
  copyAddress: "复制地址",
  allWays: "所有方式",
  byCard: "💳 银行卡支付",

  openStoryTools: "打开故事工具",
  closeStoryTools: "关闭故事工具",
  storyTools: "故事工具",
  tools: "工具",
  hideCharacter: "隐藏角色",
  showCharacter: "显示角色",
  hideMenu: "隐藏菜单",
  showMenu: "显示菜单",
  continueWithoutMe: "让讲述者不等你，自行继续",
  regenerateLastPassage: "重新生成上一段",
  removeLastExchange: "撤销上一轮对话",
  heroDeadNoOne: "主角已死——已无人可以续写",
  save: "保存",

  demoBlade: "狂怒之刃",
  demoRing: "守卫者之戒",
  demoPotion: "治疗药水",
  demoCheck: "敏捷 · d20 = 检定",
  detailsKeepPlaceholder: "讲述者应记住的细节……",
  inventoryFullPlaceholder: "物品、装备、金钱、任务道具……",
  skillsFullPlaceholder: "天赋、技能、职业特性……",
  spellsFullPlaceholder: "已准备的法术、能力、冷却说明……",
  thisComputer: "这台电脑",
  provider: "服务提供方",
  localServer: "本地服务器",
  pickModel: "——选择模型——",
  orTypeIdByHand: "或手动输入 ID",
  onlyIfServerNeeds: "仅在服务器要求时填写",
  images: "图像",
  storyWord: "故事",
  inventItem: "帮我想一个",
  audioBlocked: "浏览器窗口拦截了声音：点击页面后朗读就会继续。",

  idea: "灵感",
  stageNarrator: "讲述者",
  stageFrameWord: "画面",
  stageVoiceWord: "音色",
  stageSpeechWord: "语音",

  errorChat: "无法加载对话。",
  errorItem: "无法更新物品。",
  errorQuest: "无法修改任务。",
  errorDelete: "无法删除。",
  errorWipe: "无法清除本地数据。",
  errorLibrary: "无法加载故事库。",
  errorSettings: "无法保存设置。",
  errorImage: "无法加载图像。",
  errorCharacterSave: "无法保存角色。",
  errorCharacterUpdate: "无法更新角色。",
  errorCharacterPortrait: "无法加载角色头像。",
  errorCharacterDelete: "无法删除角色。",
  errorGenerate: "无法生成图像。",
  errorStoryStream: "故事流中断了。",
  errorStoryRequest: "无法执行故事请求。",
  errorStoryTimeout:
    "讲述者响应超时。模型可能仍在后台运行，请稍等片刻，然后重试或重新开始。",
  errorStoryCreate: "无法创建故事。",
  errorSaveChanges: "无法保存更改。",
  errorVoiceLoad: "无法加载音色。",
  errorTtsServer: "朗读服务器未运行（端口 8081）。",
  errorCharacterGenerate: "无法生成角色。",
  errorTextServer: "失败——文本服务器是否已启动？",
  errorImageTool: "图像工具出错。",
  drawingFrame: "正在绘制场景…",
  formingPassage: "正在生成下一段…",
  generatingScene: "正在生成场景……",
  imageServerRequested: "已请求启动图像服务器。",
  imageServerFailed: "无法启动图像服务器。",
  modelFolderOpened: "模型文件夹已打开。",
  modelFolderFailed: "无法打开模型文件夹。",
};

const JA: SettingsText = {
  everyStoryStartsWithALine: "物語はすべて、一行から始まる。",
  startNewStory: "新しい物語を始める",
  pickSetting: "舞台を選び、自分が誰かを伝え、物語の始まり方を選ぶ。",
  whatIsThisStory: "これはどんな物語？",
  whoAreYou: "あなたは誰？",
  name: "名前",
  optional: "（任意）",
  characterGender: "キャラクターの性別",
  genderMale: "男性",
  genderFemale: "女性",
  genderUnset: "未設定",
  beginning: "始まり方",
  narratorSetsTheScene: "語り手が場面を設定する",
  writeTheOpeningYourself: "冒頭を自分で書く",
  openingHint: "冒頭のヒント",
  openingHintPlaceholder: "例：前夜の記憶がないまま独房で目覚めるところから始めて",
  yourOpeningPassage: "あなたの書いた冒頭",
  cancel: "キャンセル",
  beginStory: "物語を始める",
  ownWorld: "自分だけの世界",
  describeYourStart: "始まりを自分で描写する",
  fillForMe: "代わりに埋めてもらう",
  thinkingUp: "考え中…",
  couldNotInvent: "思いつかなかった——もう一度試して。",
  close: "閉じる",

  genreFantasy: "ファンタジー",
  genreFantasyNote: "騎士、魔法、古い街道",
  genreFantasyWorld:
    "ハイファンタジー：争い合う王国、古の魔法、そして日暮れとともに安全ではなくなる街道。",
  genreFantasyRole: "旅する傭兵",
  genreDetective: "探偵",
  genreDetectiveNote: "雨、秘密、解けない糸口",
  genreDetectiveWorld:
    "秘密に満ちた雨の街。どの事件も、誰かが閉ざしておきたい扉だ。",
  genreDetectiveRole: "私立探偵",
  genreCyberpunk: "サイバーパンク",
  genreCyberpunkNote: "ネオン、クロム、悪い借金",
  genreCyberpunkWorld:
    "企業が支配するネオンに沈んだ巨大都市。記憶が通貨となり、誰もが誰かに借りがある。",
  genreCyberpunkRole: "燃え尽きたネットランナー",
  genrePostapoc: "ポストアポカリプス",
  genrePostapocNote: "すべての終わりのあと",
  genrePostapocWorld:
    "崩壊から何年も経ったいま、散り散りの生存者たちが廃墟をあさり、物々交換をし、昔の話を語り合う。",
  genrePostapocRole: "地図を持つ廃品回収屋",
  genreHorror: "ホラー",
  genreHorrorNote: "何かがおかしいこの場所",
  genreHorrorWorld:
    "夜が長く続き、そこで何が起きているのか住人たちが語ろうとしない、辺鄙な町。",
  genreHorrorRole: "よそ者",
  genreRomance: "ロマンス",
  genreRomanceNote: "思わぬ場所に灯る火花",
  genreRomanceWorld:
    "夏の終わりの小さな海辺の町。偶然の出会いが、やがて何か大きなものへと育っていく。",
  genreRomanceRole: "過去を抱えた新参者",

  hero: "主人公",
  details: "詳細",
  skills: "スキル",
  spells: "呪文",
  thisCharacter: "このキャラクター",
  savedCharactersAppearHere: "保存したキャラクターはここに表示される。",
  deleteThisCharacter: "このキャラクターを削除する？",
  delete: "削除",
  clear: "クリア",
  detailsPlaceholder: "短い黒髪、ボーイッシュ、乾いたユーモア……",
  inventoryPlaceholder: "鉄の短剣、ランタン、銀貨12枚……",
  skillsPlaceholder: "鍵開け、薬草学……",
  spellsPlaceholder: "修復術、魔法の手……",
  fillCharacterForMe: "代わりにキャラクターを埋めてもらう",
  removeDraftPortrait: "下書きの肖像を削除する",
  regenerateAvatar: "アバターを再生成する",
  voiceAutoNote: "自動——キャラクターごとに安定した専用の声を割り当てる。「キャラクターごとの声」使用時に有効。",
  enableMultiVoiceNote: "「キャラクターごとの声」をオンにすると、セリフがこの声で読まれる。",

  world: "世界",
  style: "スタイル",
  dndMode: "⚔️ D&Dモード（ステータス、サイコロ、記録）",
  randomEvents: "✨ ランダムイベント（祝福・呪い）",
  dice: "🎲 サイコロ",
  dice3d: "3D演出",
  diceSound: "ロール音",
  howTheStoryGoes: "✨ 物語の語られ方",
  avoidRepeats: "場面の繰り返しを避ける",
  meaningfulEnding: "意味のある結末",
  companion: "相棒の合いの手",
  narratorPrompt: "語り手のプロンプト",
  narratorPromptNote: "語り手のシステムプロンプト。空欄なら組み込みの初期設定になる。",
  fontSize: "文字サイズ",
  responseLength: "応答の長さ",
  lengthShort: "短め",
  lengthMedium: "普通",
  lengthLong: "長め",
  lengthEpic: "叙事詩級",
  language: "言語",

  imageGeneration: "画像生成",
  models: "モデル",
  size: "サイズ",
  ratio: "比率",
  autoImages: "自動画像生成",
  frameStyle: "カットのスタイル",
  frameStyleNote: "すべてのカットの先頭に付与され、物語全体の絵柄を統一する。",
  frameStylePlaceholder: "例：ダークファンタジー、油絵、くすんだ色調",
  imagePrompt: "画像プロンプト",
  imagePromptNote:
    "画像生成モデルへの指示（FLUXプロンプト自体は英語のまま）。空欄なら組み込みの初期設定になる。",
  square: "正方形",
  portrait: "縦長",
  landscape: "横長",
  stepsPerFrame: "カットあたりのステップ数",
  stepsNote: "ステップ数が少ないほどカットは速く出るが粗くなる。Krea-2 Turboは8ステップ用に蒸留されている。",
  promptStrength: "プロンプト強度",
  promptStrengthNote: "蒸留モデルでは1のままにしておくこと。",
  frameEngineReady: "カット生成エンジンは準備完了",
  frameWeightsMissing: "カット用の重みが見つからない",
  frameEngineNote: "カットはカード上で描画され、終わり次第すぐにメモリが解放される。",
  frameWeightsHint: "重みをダウンロードするか、「エンジンとクラウド」でカット生成をクラウドに切り替えて。",
  rendering: "描画中",

  autoNarration: "自動読み上げ",
  narrationVoice: "読み上げの声",
  narrationOn: "読み上げオン",
  narrationOnNote: "読み上げをオフにすればカードもクラウドも消費しない。",
  narrationParallel: "ターンと並行して読み上げる",
  narrationParallelNote: "速くなるが、カード上では余分にメモリを食う——非力なカードではオフにした方がいい。",
  volume: "音量",
  speechRate: "話す速さ",
  referenceSeconds: "声のサンプル秒数",
  referenceSecondsNote: "サンプルの何秒分をクローン生成エンジンに渡すか。",
  uploadOwnVoice: "自分の声をアップロード（.mp3）",
  loading: "読み込み中…",
  synthesis: "音声合成",
  perCharacterVoices: "キャラクターごとの声でセリフを読む",
  stopRecording: "録音を止める",

  slotWeapon: "武器",
  slotArmor: "防具",
  slotShield: "盾",
  slotTrinket: "装飾品",
  slotConsumable: "消耗品",
  slotMisc: "その他",
  unequip: "外す",
  equip: "装備する",
  equippedHint: " ・装備中（クリックで外す）",
  equipHint: " ・装備する",
  loot: "戦利品——",
  takeQuest: "受注",
  declineQuest: "断る",
  abandonQuest: "放棄",
  dead: "☠️ 死亡",

  wipeEverything: "アプリを完全に初期化する？",
  deleteAll: "すべて削除",
  deleteThisStory: "この物語を削除する？",
  copyAddress: "アドレスをコピー",
  allWays: "すべての方法",
  byCard: "💳 カードで",

  openStoryTools: "物語ツールを開く",
  closeStoryTools: "物語ツールを閉じる",
  storyTools: "物語ツール",
  tools: "ツール",
  hideCharacter: "キャラクターを隠す",
  showCharacter: "キャラクターを表示する",
  hideMenu: "メニューを隠す",
  showMenu: "メニューを表示する",
  continueWithoutMe: "自分抜きで語り手に続けてもらう",
  regenerateLastPassage: "最後の一文を再生成する",
  removeLastExchange: "直前のやり取りを取り消す",
  heroDeadNoOne: "主人公が死亡した——もう続きを書く者がいない",
  save: "保存",

  demoBlade: "怒りの刃",
  demoRing: "番人の指輪",
  demoPotion: "回復薬",
  demoCheck: "敏捷・d20＝判定",
  detailsKeepPlaceholder: "語り手が覚えておくべき詳細……",
  inventoryFullPlaceholder: "アイテム、装備、所持金、クエストアイテム……",
  skillsFullPlaceholder: "才能、技能、クラス特性……",
  spellsFullPlaceholder: "準備済みの呪文、能力、再詠唱に関するメモ……",
  thisComputer: "このコンピューター",
  provider: "プロバイダー",
  localServer: "ローカルサーバー",
  pickModel: "——モデルを選択——",
  orTypeIdByHand: "またはidを手入力",
  onlyIfServerNeeds: "サーバーが要求する場合のみ",
  images: "画像",
  storyWord: "物語",
  inventItem: "代わりに考えてもらう",
  audioBlocked: "ウィンドウが音声をブロックした：ページをクリックすれば読み上げが再開する。",

  idea: "アイデア",
  stageNarrator: "語り手",
  stageFrameWord: "カット",
  stageVoiceWord: "声",
  stageSpeechWord: "セリフ",

  errorChat: "チャットを読み込めなかった。",
  errorItem: "アイテムを更新できなかった。",
  errorQuest: "クエストを変更できなかった。",
  errorDelete: "削除できなかった。",
  errorWipe: "ローカルデータを消去できなかった。",
  errorLibrary: "物語ライブラリを読み込めなかった。",
  errorSettings: "設定を保存できなかった。",
  errorImage: "画像を読み込めなかった。",
  errorCharacterSave: "キャラクターを保存できなかった。",
  errorCharacterUpdate: "キャラクターを更新できなかった。",
  errorCharacterPortrait: "キャラクターの肖像を読み込めなかった。",
  errorCharacterDelete: "キャラクターを削除できなかった。",
  errorGenerate: "画像を生成できなかった。",
  errorStoryStream: "物語のストリームが途切れた。",
  errorStoryRequest: "物語のリクエストを実行できなかった。",
  errorStoryTimeout:
    "語り手の応答に時間がかかりすぎた。モデルは裏でまだ動いているかもしれない——少し待ってから再試行するか、最初からやり直して。",
  errorStoryCreate: "物語を作成できなかった。",
  errorSaveChanges: "変更を保存できなかった。",
  errorVoiceLoad: "声を読み込めなかった。",
  errorTtsServer: "読み上げサーバーが起動していない（ポート8081）。",
  errorCharacterGenerate: "キャラクターを生成できなかった。",
  errorTextServer: "失敗した——テキストサーバーは起動している？",
  errorImageTool: "画像ツールでエラーが発生した。",
  drawingFrame: "場面のカットを描いている…",
  formingPassage: "次の一文を生成している…",
  generatingScene: "場面を生成中……",
  imageServerRequested: "画像サーバーの起動を要求した。",
  imageServerFailed: "画像サーバーを起動できなかった。",
  modelFolderOpened: "モデルフォルダを開いた。",
  modelFolderFailed: "モデルフォルダを開けなかった。",
};

/// Каждый язык переведён целиком: подписи настроек больше нигде не подменяются чужими.
const BY_LANGUAGE: Record<Language, SettingsText> = {
  ru: RU,
  en: EN,
  es: ES,
  fr: FR,
  de: DE,
  zh: ZH,
  ja: JA,
};

export function settingsText(language: Language | undefined): SettingsText {
  return (language && BY_LANGUAGE[language]) || RU;
}
