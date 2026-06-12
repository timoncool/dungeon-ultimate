import type { LocalTextModelId, TextProvider } from "@/lib/text-models";

export type StoryRole = "user" | "assistant";

export type AspectPreset = "square" | "portrait" | "landscape";

export type ImageMode = "fast" | "slow";

export type ImageBackend = "mflux-hs" | "sdnq-hs";

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
  textProvider: TextProvider;
  localTextModel: LocalTextModelId;
  // Custom OpenAI-compatible backend (llama.cpp, LM Studio, vLLM, etc.).
  customBaseUrl: string;
  customModel: string;
  // Optional key for the custom backend, stored locally. Most local servers
  // need none. Falls back to OPENAI_COMPAT_API_KEY when blank.
  customApiKey: string;
  imageMode: ImageMode;
  imageBackend: ImageBackend;
  aspect: AspectPreset;
  autoImages: boolean;
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
  portrait?: Attachment;
  createdAt: string;
  updatedAt: string;
};
