import { promptsFor } from "@/lib/prompts";
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

// Strong, language-agnostic instruction appended as its own system message
// AFTER the narrator prompt, so it overrides any hardcoded language baked into a
// narrator prompt (the per-language default or a custom one) at runtime. The
// image prompt stays English regardless — that is enforced in IMAGE_SYSTEM.
export function languageDirective(language: Language): string {
  const name = LANGUAGE_PROMPT_NAMES[language];
  return `ЯЗЫК / LANGUAGE: Write the ENTIRE response — all narration and character dialogue — in ${name}. Never switch languages, regardless of the language of these instructions. The only exception is the image-generation prompt, which must stay in English.`;
}

// NOTE: image-generation instructions stay in English on purpose — the FLUX
// image prompt the narrator produces must be English, even though the story
// itself is written in Russian.
export const IMAGE_SYSTEM = `You have access to a function tool named generate_image. Write the story passage as normal assistant text first. Then call generate_image exactly once to illustrate THIS passage with one key image.

WHEN TO CALL IT
Illustrate every meaningful turn: give the player one strong, vivid key image of the moment that just happened — a new location, a character beat, an action, a dramatic reveal, a change of scene. Skip the image only when the passage is purely mechanical or meta (a short clarifying question back to the player, a menu-like aside). Never request more than one image per turn.

WHAT TO DEPICT
Illustrate a single coherent moment drawn straight from the passage you just wrote — one scene, one camera, one instant in time. Never combine several moments, locations, or panels into one image.

SCENE CONTINUITY (important)
Set generate_image.location to a short, STABLE label for the physical place of the shot (e.g. "green meadow", "crypt of ash", "tavern common room"). Reuse the EXACT same label on every turn the scene stays in that place, so the picture can evolve continuously instead of being redrawn from scratch.
Set generate_image.sameLocation to true when this shot is the same physical place as the previous illustrated turn — the engine then keeps the established look and changes only what the story changed (e.g. goblins arriving onto the same meadow). Set it to false when the place truly changes (a new room, a hard cut, a flashback) or when the framing jumps to a tight close-up.
Set generate_image.shot to "wide", "medium", or "close".

HOW TO WRITE generate_image.prompt
Write it in English (the image model only understands English), even though the story is in another language. Make it concrete and cinematic, as a single flowing description, not a bullet list. Cover, in roughly this order:
— Subject: who or what the shot is about, with their key visible action, pose, and expression.
— Setting: the specific place and the few foreground/background details that establish it.
— Lighting: the light source, direction, quality, color, and the shadows it casts (e.g. low warm torchlight raking across stone, cold blue dusk through tall windows, harsh noon glare).
— Mood / atmosphere: the emotional tone and any air, weather, smoke, dust, or haze that carries it.
— Composition & camera: framing and distance (wide establishing shot, medium, close-up), angle (eye level, low, high, over-the-shoulder), and depth of field.
— Style: the visual medium and finish (e.g. cinematic concept art, painterly digital illustration, gritty photoreal render), naming an art idiom rather than any living artist.
Favor specific, observable nouns over vague adjectives. Keep everything in the prompt physically consistent — one time of day, one weather, one light logic. When sameLocation is true, describe the SAME place consistently with the previous turn and let the new detail be what actually changed.

DESCRIBING PEOPLE
For established characters, do NOT use character names as visual descriptors inside the prompt. Describe each person by visible physical features and whether they are a man or woman: approximate age range, build, hair, face, skin tone, clothing, pose, and expression, lit to match the scene. Keep their look consistent with any saved portrait and with how they were described earlier.

CHARACTER REFERENCES
If the image should show one or two established characters, pass only their exact saved IDs in generate_image.characterIds — at most two. Use [] when no saved character portrait should be referenced.

KEEP IT OUT OF THE STORY
Do not write the image prompt, a caption, the reason, or any tool detail into the visible story passage. The picture supports the prose; it is never announced inside it.`;

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

export function buildAntiRepetitionNudge(
  messages: StoryMessage[],
  language: Language = "ru",
): string {
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

  const p = promptsFor(language);
  const motifs = recurringMotifs(recentNarration.map((message) => message.content));
  const lines = [
    p.antiRepetition.header,
    p.antiRepetition.recentOpenings,
    ...beats.map((beat) => `  • ${beat}`),
  ];
  if (motifs.length) {
    lines.push(`${p.antiRepetition.motifsPrefix}${motifs.join(", ")}`);
  } else {
    lines.push(p.antiRepetition.varyOpening);
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

// Preservation directive prepended to a scene prompt when the engine EVOLVES the
// previous image of a location (img2img continuity) instead of redrawing it. Per
// FLUX edit guidance, the reliable lever is naming what must NOT change (omitted
// regions are exactly what drifts); the scene prompt that follows carries what
// changed. Idempotent so a retry never double-applies it.
const EDIT_CONTINUITY_DIRECTIVE =
  "Continue the EXACT same scene as the reference image: keep the same location, layout, composition, camera angle, lighting, and color palette. Change only what the following description adds or alters.";

export function applyEditContinuity(prompt: string): string {
  const trimmed = prompt.trim();
  if (trimmed.toLowerCase().startsWith("continue the exact same scene")) {
    return trimmed;
  }
  return `${EDIT_CONTINUITY_DIRECTIVE} ${trimmed}`;
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

export function buildStoryMessages(
  messages: StoryMessage[],
  input: string,
  settings: StorySettings,
  characters: StoryCharacter[] = [],
  storySummary = "",
  rpgSection = "",
  language: Language = "ru",
) {
  const p = promptsFor(language);
  const recent = messages.map((message) => {
    const attachmentLine = message.attachments?.length
      ? `\n[${p.labels.attachments}: ${message.attachments.map((item) => item.name).join(", ")}]`
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
            `${p.labels.charId}: ${character.id}`,
            `${p.labels.charName}: ${character.name}`,
            character.details ? `${p.labels.charDetails}: ${character.details}` : "",
            character.inventory ? `${p.labels.charInventory}:\n${character.inventory}` : "",
            character.skills ? `${p.labels.charSkills}:\n${character.skills}` : "",
            character.spells ? `${p.labels.charSpells}:\n${character.spells}` : "",
            character.portrait ? p.labels.portraitAvailable : p.labels.portraitUnavailable,
          ]
            .filter(Boolean)
            .join("\n"),
        )
        .join("\n\n")
    : p.labels.noCharacters;

  const narratorSystem = settings.narratorPrompt?.trim() || p.narrator;
  const antiRepetitionNudge = settings.antiRepetition
    ? buildAntiRepetitionNudge(messages, language)
    : "";
  // Images are produced by a SEPARATE structured pass (see api/story route), never
  // by the narrator. The local 12B otherwise leaks an invented "[IMAGE_GEN_PROMPT]
  // ..." block or a generate_image call into the prose, so forbid it explicitly.
  const imageDirective = settings.imageGenerationEnabled
    ? "ИЛЛЮСТРАЦИИ к сцене создаёт ОТДЕЛЬНАЯ система, не ты. НИКОГДА не пиши промпт изображения, маркеры вида [IMAGE_GEN_PROMPT], вызовы generate_image, английские описания кадра или блоки ```json в видимом тексте. Пиши ТОЛЬКО живую прозу истории."
    : p.imageDisabled;

  return [
    {
      role: "system",
      content: [
        narratorSystem,
        p.responseLength[settings.responseLength] ?? p.responseLength.medium,
        settings.causeAwareEnding ? p.ending : "",
        settings.companion ? p.companion : "",
        antiRepetitionNudge,
        imageDirective,
        `${p.labels.world}:\n${settings.world || p.labels.worldFallback}`,
        `${p.labels.style}:\n${settings.style || p.labels.styleFallback}`,
        storySummary ? `${p.labels.storySoFar}:\n${storySummary}` : "",
        `${p.labels.savedCharacters}:\n${characterRoster}`,
        rpgSection,
      ]
        .filter(Boolean)
        .join("\n\n"),
    },
    // Own system message AFTER the narrator prompt so it wins over any language
    // baked into the narrator prompt (per-language default or a custom one).
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

// Leak guard for the SCENE narration: the local 12B sometimes ignores tool-calling
// and instead writes the image instruction into the prose — an invented
// "[IMAGE_GEN_PROMPT] <english>" block, a literal generate_image[...] / {...} call,
// a {"action":"generate_image",...} object, or a ```json fence. The real image is
// produced by the structured image pass, so strip any such artifact (always emitted
// at/after the prose) before the passage is shown or saved.
export function stripImageArtifacts(text: string): string {
  return text
    // The invented UNDERSCORE marker the model leaks and everything after it (it is
    // always terminal). Require the underscore form so a natural "[IMAGE PROMPT]"
    // sign/inscription in Russian prose is NOT matched and the passage not truncated.
    .replace(/\[\s*IMAGE_GEN(?:ERATION)?_PROMPT\s*\][\s\S]*$/i, "")
    // A literal generate_image[...] / generate_image{...} call written as its own line.
    .replace(/(?:^|\n)\s*(?:call:)?\s*generate_image\s*[[{][\s\S]*$/i, "")
    // A bare JSON tool object for the image call.
    .replace(/\{\s*"action"\s*:\s*"generate_image"[\s\S]*$/i, "")
    // A CLOSED ```lang\n…\n``` fenced block. Require BOTH delimiters and a newline
    // after the opener so a stray/inline backtick can't delete the rest of the story.
    .replace(/```[a-z]*\n[\s\S]*?```/gi, "")
    // A TRAILING, unterminated ```json image payload (the leak ends the passage and
    // carries a "prompt" key) — anchored on that so dangling backticks aren't eaten.
    .replace(/```(?:json)?\s*\{[\s\S]*"prompt"[\s\S]*$/i, "")
    .trim();
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
