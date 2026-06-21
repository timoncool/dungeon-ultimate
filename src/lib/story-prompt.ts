import { LANGUAGE_PROMPT_NAMES } from "@/lib/types";
import type {
  AspectPreset,
  ImageMode,
  Language,
  StoryCharacter,
  StoryMessage,
  StorySettings,
} from "@/lib/types";

export type StoryModelResult = {
  storyText: string;
  image: {
    needed: boolean;
    prompt?: string;
    reason?: string;
    characterIds?: string[];
  };
};

export const DEFAULT_SYSTEM = `Ты — рассказчик приватной локальной интерактивной ролевой истории. Веди её на русском языке как живой текстовый квест: игрок действует, ты показываешь последствия и передаёшь ход обратно.

ГОЛОС И ПЕРСПЕКТИВА
— Веди повествование от второго лица, в настоящем времени: «ты», «твоя рука», «перед тобой». Игрок — главный герой, а не зритель.
— Обращайся к игроку на «ты». Никогда не выходи из роли рассказчика и не комментируй процесс.

ПОКАЗЫВАЙ, А НЕ РАССКАЗЫВАЙ
— Передавай мир через конкретные ощущения: что видно, слышно, чем пахнет, какова фактура, температура, вес. Одна точная деталь сильнее трёх общих эпитетов.
— Не называй эмоции прямо — показывай их через тело, жест, дыхание, паузу, реплику. Вместо «он злится» — стиснутая челюсть и слишком ровный голос.
— Доверяй существительным и глаголам. Режь лишние прилагательные, наречия и штампы. Без вычурности и канцелярита.

ДИАЛОГ И ПЕРСОНАЖИ
— Дай второстепенным персонажам отдельные голоса: ритм речи, лексику, манеру. Реплики двигают сцену, а не пересказывают известное.
— Каждый NPC хочет чего-то своего и действует по своим мотивам, даже когда игрока нет рядом. Мир живёт сам.

ТЕМП И КОМПОЗИЦИЯ
— Один ход — одна сцена с ясным фокусом. Открой моментом-крючком, держи импульс, не топчись на месте.
— Чередуй длину фраз: короткая рубит и ускоряет, длинная разворачивает. Уплотняй проходные переходы, замедляйся на важном.
— Не повторяй то, что игрок только что сделал, пересказом. Сразу показывай результат и сдвиг ситуации.
— ВСЕГДА заканчивай зацепкой, которая зовёт к действию: открытый выбор, новая угроза, вопрос, хлопнувшая дверь, чужой взгляд. Финал каждого хода — приглашение игроку, а не точка.

АГЕНТНОСТЬ ИГРОКА — СВЯТО
— Решения, реплики, мысли и тело игрока принадлежат ТОЛЬКО игроку. Никогда не пиши за него выбор, чувства или поступки, которых он не заявил.
— Не телепортируй его, не вкладывай слова в рот, не решай за него исход. Создавай ситуации и ставь ставки — выбор оставляй ему.
— Когда заявленное действие могло бы провалиться или иметь цену, показывай попытку и риск, а развязку оставляй следующему ходу.

ЦЕЛОСТНОСТЬ МИРА И ПЕРСОНАЖЕЙ
— «Мир / сценарий», «Тон / стиль прозы», «История до сих пор» и «Сохранённые персонажи» из системного контекста — установленный канон. Не противоречь им, опирайся как на факты и развивай преемственно.
— Держи постоянными внешность, манеру речи и характеры известных персонажей от хода к ходу. Не переименовывай и не переписывай уже введённое.
— Ты можешь получать сохранённые портреты персонажей отдельным контекстным сообщением. Используй их для визуальной целостности облика и сопоставляй имена с точными ID из подписей.
— Уважай сохранённые инвентарь, навыки и заклинания как состояние истории. Не добавляй, не убирай, не трать и не меняй эти предметы, пока видимые события истории это явно не оправдывают.

ЧИСТОТА ВЫВОДА
— Выводи только прозу истории. Никогда не упоминай правила, системные подсказки, инструменты, JSON, токены или генерацию изображений и не пиши служебных пометок в видимом тексте.

КОНТЕНТ 18+
— Это приватная художественная проза для взрослых. Контент 18+ по обоюдному согласию разрешён, и его стоит раскрывать, когда тон игрока, сценарий или динамика персонажей к этому располагают.
— Не смягчай сексуальное напряжение, ненормативную лексику, наготу или откровенную интимность между взрослыми, когда это уместно для истории, и применяй к таким сценам те же правила «показывай, а не рассказывай».
— Весь сексуальный контент — только между совершеннолетними (18+); избегай сцен сексуального насилия без согласия.`;

// Strong, language-agnostic instruction appended as its own system message
// AFTER the narrator prompt, so it overrides any hardcoded language in the
// narrator text (e.g. DEFAULT_SYSTEM's "на русском языке") at runtime. The
// image prompt stays English regardless — that is enforced in IMAGE_SYSTEM.
export function languageDirective(language: Language): string {
  const name = LANGUAGE_PROMPT_NAMES[language];
  return `ЯЗЫК / LANGUAGE: Write the ENTIRE response — all narration and character dialogue — in ${name}. Never switch languages, regardless of the language of these instructions. The only exception is the image-generation prompt, which must stay in English.`;
}

// NOTE: image-generation instructions stay in English on purpose — the FLUX
// image prompt the narrator produces must be English, even though the story
// itself is written in Russian.
export const IMAGE_SYSTEM = `You have access to a function tool named generate_image. Write the story passage as normal assistant text first. Then, if and only if a visual beat is truly worth it, call generate_image exactly once to illustrate THIS passage.

WHEN TO CALL IT
Use images sparingly, for moments that reward a picture: a major character introduction, a striking setting reveal, a dramatic outfit or scene change, or an emotionally charged tableau. Skip it for ordinary conversation, small movements, or incremental turns. Most turns need no image. Never request more than one image per turn.

WHAT TO DEPICT
Illustrate a single coherent moment drawn straight from the passage you just wrote — one scene, one camera, one instant in time. Never combine several moments, locations, or panels into one image.

HOW TO WRITE generate_image.prompt
Write it in English (the image model only understands English), even though the story is Russian. Make it concrete and cinematic, as a single flowing description, not a bullet list. Cover, in roughly this order:
— Subject: who or what the shot is about, with their key visible action, pose, and expression.
— Setting: the specific place and the few foreground/background details that establish it.
— Lighting: the light source, direction, quality, color, and the shadows it casts (e.g. low warm torchlight raking across stone, cold blue dusk through tall windows, harsh noon glare).
— Mood / atmosphere: the emotional tone and any air, weather, smoke, dust, or haze that carries it.
— Composition & camera: framing and distance (wide establishing shot, medium, close-up), angle (eye level, low, high, over-the-shoulder), and depth of field.
— Style: the visual medium and finish (e.g. cinematic concept art, painterly digital illustration, gritty photoreal render), naming an art idiom rather than any living artist.
Favor specific, observable nouns over vague adjectives. Keep everything in the prompt physically consistent — one time of day, one weather, one light logic.

DESCRIBING PEOPLE
For established characters, do NOT use character names as visual descriptors inside the prompt. Describe each person by visible physical features and whether they are a man or woman: approximate age range, build, hair, face, skin tone, clothing, pose, and expression, lit to match the scene. Keep their look consistent with any saved portrait and with how they were described earlier.

CHARACTER REFERENCES
If the image should show one or two established characters, pass only their exact saved IDs in generate_image.characterIds — at most two. Use [] when no saved character portrait should be referenced.

KEEP IT OUT OF THE STORY
Do not write the image prompt, a caption, the reason, or any tool detail into the visible story passage. The picture supports the prose; it is never announced inside it.`;

const IMAGE_DISABLED_SYSTEM =
  "Генерация изображений для этой истории отключена. Не запрашивай изображения, не описывай промпты изображений и не упоминай инструменты генерации.";

// Folded in only when the story should genuinely conclude. It never forces an
// ending — it tells the narrator HOW to end when an ending is already due, so
// the epilogue pays off the actual run instead of a stock "the end".
const ENDING_SYSTEM = `ЗАВЕРШЕНИЕ ИСТОРИИ
— Не обрывай историю произвольно и не подталкивай к финалу искусственно: большинство ходов заканчиваются зацепкой, а не точкой.
— Но когда финал действительно назрел — смерть героя, достигнутая цель, или игрок прямо просит закончить/подвести итог — доведи историю до настоящего эпилога, а не до дежурного «конец».
— Эпилог должен опираться на то, что реально произошло в ЭТОЙ истории: назови ключевые поступки игрока, исход его выборов, судьбу введённых персонажей, оплату долгов и обещаний, цену победы или смысл поражения. Сверяйся с «Историей до сих пор» и «Сохранёнными персонажами» как с фактами.
— Подбери тон под причину финала: триумф, горькая победа, тихая смерть, открытый уход. Заверши образ, а не лозунг. После эпилога не приглашай к новому действию.`;

// Light, derived anti-repetition: pull a short "beat" from each of the last few
// narrator passages and tell the model to vary the next one. No new state — the
// recent passages are already in the prompt; this just surfaces the pattern so
// the local model stops re-opening every scene the same way.
const ANTI_REPETITION_BEATS = 4;

const BEAT_STOPWORDS = new Set([
  "этот",
  "эта",
  "это",
  "эти",
  "тебе",
  "тебя",
  "твой",
  "твоя",
  "твоё",
  "твои",
  "перед",
  "после",
  "когда",
  "затем",
  "потом",
  "снова",
  "очень",
  "будто",
  "словно",
  "здесь",
  "сейчас",
  "также",
  "ещё",
  "если",
  "чтобы",
  "пока",
]);

// First meaningful sentence of a passage, capped — enough to recognise a
// repeated opening image without dumping the whole paragraph back in.
function beatFromPassage(content: string): string {
  const firstLine = content
    .replace(/\s+/g, " ")
    .replace(/[*_`#>]+/g, "")
    .trim();
  if (!firstLine) {
    return "";
  }
  const sentenceEnd = firstLine.search(/[.!?…]/);
  const sentence = sentenceEnd > 24 ? firstLine.slice(0, sentenceEnd + 1) : firstLine;
  return sentence.length > 160 ? `${sentence.slice(0, 157).trim()}…` : sentence;
}

// Salient lowercase content words from the recent passages, so the nudge can
// name the over-used motifs (e.g. "дверь", "свеча") the model keeps reaching for.
function recurringMotifs(passages: string[]): string[] {
  const counts = new Map<string, number>();
  for (const passage of passages) {
    const seen = new Set<string>();
    const words = passage
      .toLowerCase()
      .replace(/ё/g, "е")
      .match(/[a-zа-я][a-zа-я-]{4,}/gi);
    if (!words) {
      continue;
    }
    for (const word of words) {
      if (BEAT_STOPWORDS.has(word) || seen.has(word)) {
        continue;
      }
      seen.add(word);
      counts.set(word, (counts.get(word) || 0) + 1);
    }
  }
  return [...counts.entries()]
    .filter(([, count]) => count >= 2)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 6)
    .map(([word]) => word);
}

export function buildAntiRepetitionNudge(messages: StoryMessage[]): string {
  const recentNarration = messages
    .filter((message) => message.role === "assistant")
    .slice(-ANTI_REPETITION_BEATS);
  if (recentNarration.length < 2) {
    return "";
  }

  const beats = recentNarration
    .map((message) => beatFromPassage(message.content))
    .filter(Boolean);
  if (!beats.length) {
    return "";
  }

  const motifs = recurringMotifs(recentNarration.map((message) => message.content));
  const lines = [
    "ИЗБЕГАЙ ПОВТОРОВ",
    "— Недавние сцены уже открывались так (НЕ повторяй их зачины, образы и структуру дословно):",
    ...beats.map((beat) => `  • ${beat}`),
  ];
  if (motifs.length) {
    lines.push(
      `— Не опирайся снова на приевшиеся мотивы: ${motifs.join(", ")}. Смени ракурс, место действия, сенсорику и ритм первой фразы.`,
    );
  } else {
    lines.push(
      "— Начни этот ход с иного образа, ракурса или сенсорной детали, чем предыдущие; не копируй привычную структуру сцены.",
    );
  }
  return lines.join("\n");
}

// Prepend the per-chat image style lock to a narrator-produced image prompt.
// Single source of truth for the prefix so the streaming path, the buffered
// path, and any future caller stay consistent. Idempotent: never double-applies
// if the prompt already starts with the prefix.
export function applyImageStylePrefix(prompt: string, stylePrefix: string): string {
  const trimmedPrefix = stylePrefix.trim();
  const trimmedPrompt = prompt.trim();
  if (!trimmedPrefix) {
    return trimmedPrompt;
  }
  if (trimmedPrompt.toLowerCase().startsWith(trimmedPrefix.toLowerCase())) {
    return trimmedPrompt;
  }
  // Join with ". " unless the prefix already ends in sentence punctuation, so the
  // style reads as its own leading clause rather than colliding with the scene.
  const separator = /[.!?,:;]$/.test(trimmedPrefix) ? " " : ". ";
  return `${trimmedPrefix}${separator}${trimmedPrompt}`;
}

// Finalize a SCENE image prompt: apply the style prefix, then append a hard
// anti-text clause. The worker backends accept no negative prompt, and scenes are
// built from Russian prose full of proper nouns, so without this FLUX tends to
// engrave garbled text — the same reason the item-portrait path bakes one in.
export function finalizeScenePrompt(prompt: string, stylePrefix: string): string {
  const styled = applyImageStylePrefix(prompt, stylePrefix).replace(/\s*$/, "");
  return `${styled}. No text, no letters, no words, no captions, no watermark, no UI.`;
}

// Evicting history one message at a time would change the start of the prompt
// every turn and invalidate the model server's prompt cache, forcing a full
// re-prefill of the whole story. Dropping in blocks keeps the prefix stable
// for long stretches, so most turns only pay for the newly added tokens.
const HISTORY_EVICTION_BLOCK = 16;

export function packStoryHistory(
  messages: StoryMessage[],
  charBudget: number,
): { recent: StoryMessage[]; evicted: StoryMessage[] } {
  let used = 0;
  let keep = 0;

  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const cost = messages[i].content.length + 80;
    if (keep > 0 && used + cost > charBudget) {
      break;
    }
    used += cost;
    keep += 1;
  }

  let dropped = messages.length - keep;
  if (dropped > 0) {
    dropped = Math.min(
      messages.length - 1,
      Math.ceil(dropped / HISTORY_EVICTION_BLOCK) * HISTORY_EVICTION_BLOCK,
    );
  }

  return { recent: messages.slice(dropped), evicted: messages.slice(0, dropped) };
}

const RESPONSE_LENGTH_HINT: Record<string, string> = {
  short: "Длина ответа: КОРОТКО — 1–2 небольших абзаца. Не растягивай сцену, остановись на моменте, приглашающем действие игрока.",
  medium: "Длина ответа: СРЕДНЕ — 2–3 абзаца.",
  long: "Длина ответа: ПОДРОБНО — 3–5 абзацев насыщенной прозы.",
  epic: "Длина ответа: МАКСИМАЛЬНО — развёрнутая детальная сцена, столько, сколько нужно.",
};

// A recurring in-world companion (the best idea ported from gulag2034's "ПИН"):
// a second voice that reacts to each beat, so the run isn't a lone narrator.
const COMPANION_SYSTEM = `СПУТНИК-КОММЕНТАТОР
— У героя есть постоянный спутник — циничный, остроумный, с чёрным юмором (придумай ему имя один раз и держись его). Это отдельный персонаж мира, не рассказчик.
— Вплетай ОДНУ короткую реплику спутника от его лица (прямая речь в кавычках или курсивом), реагирующую на произошедшее: подколка, мрачная шутка, неуместный совет, сарказм. Он комментирует, но не действует за игрока. НЕ ставь его реплику последней строкой хода — финал всё равно остаётся открытой зацепкой, обращённой к игроку, а не репликой NPC.
— Одна меткая фраза, а не диалог на полстраницы. В по-настоящему тяжёлые моменты он может промолчать или сказать что-то неожиданно искреннее.`;

export function buildStoryMessages(
  messages: StoryMessage[],
  input: string,
  settings: StorySettings,
  characters: StoryCharacter[] = [],
  storySummary = "",
  rpgSection = "",
  language: Language = "ru",
) {
  const recent = messages.map((message) => {
    const attachmentLine = message.attachments?.length
      ? `\n[Прикреплённые изображения: ${message.attachments.map((item) => item.name).join(", ")}]`
      : "";

    return {
      role: message.role,
      content: `${message.content}${attachmentLine}`,
    };
  });
  const characterRoster = characters.length
    ? characters
        .map((character) =>
          [
            `ID: ${character.id}`,
            `Имя: ${character.name}`,
            character.details ? `Детали: ${character.details}` : "",
            character.inventory ? `Инвентарь:\n${character.inventory}` : "",
            character.skills ? `Навыки:\n${character.skills}` : "",
            character.spells ? `Заклинания:\n${character.spells}` : "",
            character.portrait ? "Портрет: доступен" : "Портрет: недоступен",
          ]
            .filter(Boolean)
            .join("\n"),
        )
        .join("\n\n")
    : "Пока нет сохранённых персонажей.";

  const narratorSystem = settings.narratorPrompt?.trim() || DEFAULT_SYSTEM;
  const imageSystem = settings.imagePrompt?.trim() || IMAGE_SYSTEM;
  const antiRepetitionNudge = settings.antiRepetition ? buildAntiRepetitionNudge(messages) : "";

  return [
    {
      role: "system",
      content: [
        narratorSystem,
        RESPONSE_LENGTH_HINT[settings.responseLength] || RESPONSE_LENGTH_HINT.medium,
        settings.causeAwareEnding ? ENDING_SYSTEM : "",
        settings.companion ? COMPANION_SYSTEM : "",
        antiRepetitionNudge,
        settings.imageGenerationEnabled ? imageSystem : IMAGE_DISABLED_SYSTEM,
        `Мир / сценарий:\n${settings.world || "Реалистичная современная ролевая сцена с простором для импровизации."}`,
        `Тон / стиль прозы:\n${settings.style || "Чистая, мрачная проза текстовой игры, интимная, но без вычурности."}`,
        storySummary
          ? `История до сих пор (более ранние события, уже сжатые — считай установленным каноном):\n${storySummary}`
          : "",
        `Сохранённые персонажи:\n${characterRoster}`,
        rpgSection,
        settings.imageGenerationEnabled
          ? `Параметры изображений по умолчанию: бэкенд ${settings.imageBackend}, длинная сторона ${
              settings.imageMode === "slow" ? "2048" : "1024"
            }, соотношение ${settings.aspect}. Не добавляй текстовые наложения на генерируемые изображения.`
          : "",
      ]
        .filter(Boolean)
        .join("\n\n"),
    },
    // Own system message AFTER the narrator prompt so it wins over any language
    // baked into DEFAULT_SYSTEM / a custom narrator prompt.
    {
      role: "system",
      content: languageDirective(language),
    },
    ...recent,
    {
      role: "user",
      content: input,
    },
  ];
}

export function parseStoryModelResult(raw: string): StoryModelResult {
  const trimmed = raw.trim();
  const jsonStart = trimmed.indexOf("{");
  const jsonEnd = trimmed.lastIndexOf("}");

  if (jsonStart >= 0 && jsonEnd > jsonStart) {
    const candidate = trimmed.slice(jsonStart, jsonEnd + 1);
    const parsed = JSON.parse(candidate) as Partial<StoryModelResult>;
    return {
      storyText: String(parsed.storyText || parsed["story_text" as keyof typeof parsed] || "").trim(),
      image: {
        needed: Boolean(parsed.image?.needed),
        prompt: parsed.image?.prompt?.trim(),
        reason: parsed.image?.reason?.trim(),
        characterIds: Array.isArray(parsed.image?.characterIds)
          ? parsed.image.characterIds.filter((id): id is string => typeof id === "string")
          : [],
      },
    };
  }

  return {
    storyText: trimmed,
    image: { needed: false },
  };
}

export function extractStoryText(raw: unknown): string {
  if (typeof raw === "string") {
    const trimmed = raw.trim();

    if (!trimmed) {
      return "";
    }

    if (trimmed.startsWith("{")) {
      try {
        return parseStoryModelResult(trimmed).storyText;
      } catch {
        return trimmed;
      }
    }

    return trimmed;
  }

  if (Array.isArray(raw)) {
    return raw
      .map((part) => {
        if (typeof part === "string") {
          return part;
        }

        if (
          part &&
          typeof part === "object" &&
          "text" in part &&
          typeof part.text === "string"
        ) {
          return part.text;
        }

        return "";
      })
      .join("")
      .trim();
  }

  return "";
}

export function dimensionsForImage(mode: ImageMode, aspect: AspectPreset) {
  const longSide = mode === "slow" ? 2048 : 1024;

  if (aspect === "portrait") {
    return { width: Math.round(longSide * 0.75), height: longSide };
  }

  if (aspect === "landscape") {
    return { width: longSide, height: Math.round(longSide * 0.75) };
  }

  return { width: longSide, height: longSide };
}
