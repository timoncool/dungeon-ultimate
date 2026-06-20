import type { LocalTextModelId, TextProvider } from "@/lib/text-models";

export type StoryRole = "user" | "assistant";

export type AspectPreset = "square" | "portrait" | "landscape";

export type ImageMode = "fast" | "slow";

export type ImageBackend = "mflux-hs" | "sdnq-hs" | "flux-uncensored";

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
};

export type StoryMessage = {
  id: string;
  role: StoryRole;
  content: string;
  createdAt: string;
  attachments?: Attachment[];
  imageRequest?: ImageRequest;
  generatedImage?: GeneratedImage;
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
