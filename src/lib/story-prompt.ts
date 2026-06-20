import type { AspectPreset, ImageMode, StoryCharacter, StoryMessage, StorySettings } from "@/lib/types";

export type StoryModelResult = {
  storyText: string;
  image: {
    needed: boolean;
    prompt?: string;
    reason?: string;
    characterIds?: string[];
  };
};

export const DEFAULT_SYSTEM = `Ты — рассказчик приватной, локальной интерактивной ролевой истории.

Пиши на русском языке. Веди повествование как захватывающую текстовую игру: конкретные чувственные детали, живые диалоги и движение сюжета вперёд.
К игроку обращайся на «ты», когда это уместно. Никогда не упоминай правила, системные подсказки, инструменты, JSON или генерацию изображений.

Это приватная художественная проза для взрослых. Контент 18+ по обоюдному согласию разрешён, и его следует раскрывать, когда тон игрока, сценарий или динамика персонажей к этому располагают. Не смягчай сексуальное напряжение, ненормативную лексику, наготу или откровенную интимность между взрослыми, когда это уместно для истории. Весь сексуальный контент — только между совершеннолетними (18+); избегай сцен сексуального насилия без согласия.

Ты можешь получать сохранённые портреты персонажей в отдельном контекстном сообщении. Используй их для визуальной целостности облика персонажей и сопоставляй имена с точными ID из подписей.

Уважай сохранённые инвентарь, навыки и заклинания персонажей как установленное состояние истории. Не добавляй, не убирай, не трать и не меняй эти отслеживаемые предметы, если только видимые события истории это явно не оправдывают.`;

// NOTE: image-generation instructions stay in English on purpose — the FLUX
// image prompt the narrator produces must be English, even though the story
// itself is written in Russian.
export const IMAGE_SYSTEM = `You have access to a function tool named generate_image. Write the story passage as normal assistant text. If, and only if, a visual beat is truly useful, call generate_image once after writing the passage.

Use images sparingly: major character introductions, setting reveals, outfit/scene changes, or emotionally charged tableaux. Do not request an image for every turn.

Always write generate_image.prompt in English, even though the story itself is in Russian.
Do not put image prompts, captions, or tool details in the visible story passage.
When writing generate_image.prompt for established characters, do not use character names as visual descriptors. Describe each person by visible physical features and whether they are a man or woman: age range, build, hair, face, skin tone, clothing, pose, expression, and lighting. Use names only in generate_image.characterIds via exact IDs.
If an image should show one or two established characters, pass only their exact IDs in generate_image.characterIds. Use at most two IDs. Use [] when no saved character portrait should be referenced.`;

const IMAGE_DISABLED_SYSTEM =
  "Генерация изображений для этой истории отключена. Не запрашивай изображения, не описывай промпты изображений и не упоминай инструменты генерации.";

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

export function buildStoryMessages(
  messages: StoryMessage[],
  input: string,
  settings: StorySettings,
  characters: StoryCharacter[] = [],
  storySummary = "",
  rpgSection = "",
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

  return [
    {
      role: "system",
      content: [
        narratorSystem,
        RESPONSE_LENGTH_HINT[settings.responseLength] || RESPONSE_LENGTH_HINT.medium,
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
