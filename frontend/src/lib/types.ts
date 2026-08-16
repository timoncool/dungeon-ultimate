import type { LocalTextModelId, TextProvider } from "@/lib/text-models";
import type { GameEvent, RpgSnapshot } from "@/lib/rpg/types";

export type StoryRole = "user" | "assistant";

export type AspectPreset = "square" | "portrait" | "landscape";

export type ImageMode = "fast" | "slow";

export type ImageBackend = "mflux-hs" | "sdnq-hs" | "flux-uncensored";

// Camera distance the narrator picks for a shot. A hard change of shot (e.g. wide
// vista -> tight close-up) is treated as a new framing, so the scene engine
// regenerates rather than editing the previous wide establishing image.
export type ImageShot = "wide" | "medium" | "close";

export const PROSE_SIZE_VALUES = [
  "tiny",
  "xsmall",
  "small",
  "medium",
  "large",
  "xlarge",
  "xxlarge",
  "huge",
  "giant",
] as const;

export type ProseSize = (typeof PROSE_SIZE_VALUES)[number];

export function isProseSize(value: unknown): value is ProseSize {
  return typeof value === "string" && PROSE_SIZE_VALUES.includes(value as ProseSize);
}

export const RESPONSE_LENGTH_VALUES = ["short", "medium", "long", "epic"] as const;

export type ResponseLength = (typeof RESPONSE_LENGTH_VALUES)[number];

export function isResponseLength(value: unknown): value is ResponseLength {
  return typeof value === "string" && RESPONSE_LENGTH_VALUES.includes(value as ResponseLength);
}

// Story language: the narrator, the "surprise me" suggestions, the quick-action
// chips and the TTS all speak the player's chosen language.
export const LANGUAGE_VALUES = ["ru", "en", "es", "fr", "de", "zh", "ja"] as const;

export type Language = (typeof LANGUAGE_VALUES)[number];

export function isLanguage(value: unknown): value is Language {
  return typeof value === "string" && LANGUAGE_VALUES.includes(value as Language);
}

// Native names for the language picker.
export const LANGUAGE_LABELS: Record<Language, string> = {
  ru: "Русский",
  en: "English",
  es: "Español",
  fr: "Français",
  de: "Deutsch",
  zh: "中文",
  ja: "日本語",
};

// How each language is named TO the model, with a native sample so it commits.
export const LANGUAGE_PROMPT_NAMES: Record<Language, string> = {
  ru: "Russian (русский)",
  en: "English",
  es: "Spanish (español)",
  fr: "French (français)",
  de: "German (Deutsch)",
  zh: "Simplified Chinese (简体中文)",
  ja: "Japanese (日本語)",
};

// BCP-47-ish code the TTS worker expects (best-effort; falls back to the voice).
export const LANGUAGE_TTS_CODES: Record<Language, string> = {
  ru: "ru",
  en: "en",
  es: "es",
  fr: "fr",
  de: "de",
  zh: "zh",
  ja: "ja",
};

export type Attachment = {
  id: string;
  name: string;
  type: string;
  url: string;
  dataUrl?: string;
};

export type ImageRequest = {
  needed: boolean;
  prompt?: string;
  mode?: ImageMode;
  backend?: ImageBackend;
  aspect?: AspectPreset;
  reason?: string;
  characterIds?: string[];
  // Scene continuity hints the narrator emits with each image. `location` is a
  // short stable label for the physical place (reused verbatim while the scene
  // stays there); `sameLocation` says this shot is the same place as the previous
  // illustrated turn (so the engine evolves the established image instead of
  // redrawing); `shot` is the camera distance. Resolved server-side into an
  // edit-vs-fresh decision against the per-chat scene state.
  location?: string;
  sameLocation?: boolean;
  shot?: ImageShot;
};

export type StoryMessage = {
  id: string;
  role: StoryRole;
  content: string;
  createdAt: string;
  attachments?: Attachment[];
  imageRequest?: ImageRequest;
  generatedImage?: GeneratedImage;
  rpgSnapshot?: RpgSnapshot; // pre-turn RPG state, for Retry/Erase rollback
  events?: GameEvent[]; // this turn's resolved game events, for inline event cards (client-side)
};

export type GeneratedImage = {
  id: string;
  url: string;
  prompt: string;
  mode: ImageMode;
  backend?: ImageBackend;
  aspect: AspectPreset;
  width: number;
  height: number;
  elapsedSeconds?: number;
  seed?: number;
  warnings?: string[];
  // Scene continuity bookkeeping (set server-side). `sceneLocation` is the
  // normalized place this image belongs to; `editedFrom` is the URL of the prior
  // scene image this one evolved from (null on a fresh establishing shot), for
  // the gallery/debug and to trace the edit chain.
  sceneLocation?: string;
  editedFrom?: string;
};

export type StorySettings = {
  world: string;
  style: string;
  // Editable system prompts. Blank = fall back to the built-in DEFAULT_SYSTEM /
  // IMAGE_SYSTEM in story-prompt.ts.
  narratorPrompt: string;
  imagePrompt: string;
  // Prepended verbatim to every image prompt the narrator produces, so the art
  // keeps one consistent look (medium, palette, era) across a whole story.
  // Blank = no style lock. Folded in server-side, so it rides along on the
  // persisted imageRequest.prompt and survives retries.
  imageStylePrefix: string;
  // Fold a short rolling list of the last few scene "beats" into the system
  // prompt and tell the narrator to vary imagery/structure. Counters the local
  // model's tendency to loop the same opening and setting turn after turn.
  antiRepetition: boolean;
  // When the story genuinely ends (death, goal reached, the player asks to wrap
  // up), have the narrator write an epilogue that pays off what actually
  // happened — using the saved characters + story-so-far — instead of a generic
  // "the end".
  causeAwareEnding: boolean;
  // Opt-in: let dialogue lines map to per-character voices for TTS. The default
  // single-voice narration path (settings.voice) is unaffected when this is off.
  multiVoice: boolean;
  // A recurring snarky in-world companion who adds a short aside each passage.
  companion: boolean;
  textProvider: TextProvider;
  localTextModel: LocalTextModelId;
  // Any OpenAI-compatible backend (llama.cpp, LM Studio, vLLM, OpenRouter, a
  // remote Ollama). Set in-app. The key is optional and stored locally; most
  // local servers need none, and it falls back to env when blank.
  customBaseUrl: string;
  customModel: string;
  customApiKey: string;
  imageMode: ImageMode;
  imageBackend: ImageBackend;
  aspect: AspectPreset;
  imageGenerationEnabled: boolean;
  autoImages: boolean;
  rpgEnabled: boolean;
  randomEvents: boolean;
  diceEnabled: boolean;
  diceSound: boolean;
  diceVolume: number;
  proseSize: ProseSize;
  responseLength: ResponseLength;
  language: Language;
  voice: string;
  autoplay: boolean;
  ttsVolume: number;
  ttsSpeed: number;
};

export type StoryChatSummary = {
  id: string;
  title: string;
  settings: StorySettings;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
  lastMessagePreview?: string;
};

export type StoryChat = StoryChatSummary & {
  messages: StoryMessage[];
  characters: StoryCharacter[];
};

export type StoryCharacter = {
  id: string;
  chatId: string;
  name: string;
  details: string;
  inventory: string;
  skills: string;
  spells: string;
  portrait?: Attachment;
  // Optional per-character TTS voice id (a voice-pack name or an uploaded clone
  // id, same id space as POST /api/tts { voice }). When set and multi-voice is
  // enabled, this character's dialogue can be read in this voice; when blank the
  // narrator's single voice (StorySettings.voice) is used. Never required.
  voice?: string;
  createdAt: string;
  updatedAt: string;
};
