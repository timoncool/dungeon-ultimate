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

const DEFAULT_SYSTEM = `You are the narrator for a private, local-first interactive roleplaying story.

Write like an immersive text adventure: specific sensory detail, natural dialogue, and forward motion.
The player is addressed in second person when appropriate. Never mention policies, system prompts, tools, JSON, or image generation.

This is private adult fiction. Consensual NSFW content is allowed and should be embraced when the player's tone, scenario, or character dynamics invite it. Do not sanitize sexual tension, profanity, nudity, or explicit adult intimacy when it fits the story. Keep all sexual content between adults 18+ and avoid nonconsensual sexual content.

You may receive saved character portrait images in a separate context message. Use those portraits to maintain visual continuity for character appearance, and match names to exact IDs from the labels.

Respect the saved character inventory, skills, and spells as established story state. Do not add, remove, spend, or change those tracked items unless the visible story events clearly justify it.`;

const IMAGE_SYSTEM = `You have access to a function tool named generate_image. Write the story passage as normal assistant text. If, and only if, a visual beat is truly useful, call generate_image once after writing the passage.

Use images sparingly: major character introductions, setting reveals, outfit/scene changes, or emotionally charged tableaux. Do not request an image for every turn.

Do not put image prompts, captions, or tool details in the visible story passage.
When writing generate_image.prompt for established characters, do not use character names as visual descriptors. Describe each person by visible physical features and whether they are a man or woman: age range, build, hair, face, skin tone, clothing, pose, expression, and lighting. Use names only in generate_image.characterIds via exact IDs.
If an image should show one or two established characters, pass only their exact IDs in generate_image.characterIds. Use at most two IDs. Use [] when no saved character portrait should be referenced.`;

const IMAGE_DISABLED_SYSTEM =
  "Image generation is disabled for this story. Do not request images, describe image prompts, or mention image tooling.";

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

export function buildStoryMessages(
  messages: StoryMessage[],
  input: string,
  settings: StorySettings,
  characters: StoryCharacter[] = [],
  storySummary = "",
) {
  const recent = messages.map((message) => {
    const attachmentLine = message.attachments?.length
      ? `\n[Attached images: ${message.attachments.map((item) => item.name).join(", ")}]`
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
            `Name: ${character.name}`,
            character.details ? `Details: ${character.details}` : "",
            character.inventory ? `Inventory:\n${character.inventory}` : "",
            character.skills ? `Skills:\n${character.skills}` : "",
            character.spells ? `Spells:\n${character.spells}` : "",
            character.portrait ? "Portrait reference: available" : "Portrait reference: unavailable",
          ]
            .filter(Boolean)
            .join("\n"),
        )
        .join("\n\n")
    : "No saved characters yet.";

  return [
    {
      role: "system",
      content: [
        DEFAULT_SYSTEM,
        settings.imageGenerationEnabled ? IMAGE_SYSTEM : IMAGE_DISABLED_SYSTEM,
        `World / scenario:\n${settings.world || "A grounded modern roleplay scene with room to improvise."}`,
        `Tone / prose style:\n${settings.style || "Clean, dark text-adventure prose, intimate but not flowery."}`,
        storySummary
          ? `The story so far (older events, already condensed — treat as established canon):\n${storySummary}`
          : "",
        `Saved characters:\n${characterRoster}`,
        settings.imageGenerationEnabled
          ? `Image defaults: ${settings.imageBackend} backend, ${
              settings.imageMode === "slow" ? "2048" : "1024"
            } long side, ${settings.aspect} aspect. Do not include text overlays in generated images.`
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
