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

/// Пока переведены русский и английский: остальные языки берут английский, а не остаются
/// с русскими подписями посреди своего текста. Доводятся следующим заходом.
const BY_LANGUAGE: Record<Language, SettingsText> = {
  ru: RU,
  en: EN,
  es: EN,
  fr: EN,
  de: EN,
  zh: EN,
  ja: EN,
};

export function settingsText(language: Language | undefined): SettingsText {
  return (language && BY_LANGUAGE[language]) || RU;
}
