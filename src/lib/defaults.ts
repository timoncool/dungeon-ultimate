import { DEFAULT_LOCAL_TEXT_MODEL } from "@/lib/text-models";
import { DEFAULT_SYSTEM, IMAGE_SYSTEM } from "@/lib/story-prompt";
import type { StorySettings } from "@/lib/types";

export const DEFAULT_CHAT_TITLE = "Untitled story";

export const DEFAULT_STORY_SETTINGS: StorySettings = {
  world:
    "Сцена интерактивной прозы с острым диалогом, высокими ставками и пространством для управления сюжетом игроком.",
  style:
    "Классическая текстовая авантюрная нарратология: прямое второе лицо, яркая, но сдержанная проза, естественный диалог и без витиеватого описания.",
  narratorPrompt: DEFAULT_SYSTEM,
  imagePrompt: IMAGE_SYSTEM,
  // Custom (OpenAI-compatible) is the primary text path; the bundled local
  // OpenAI-compatible Gemma server makes Ollama unnecessary. The env-driven
  // configuredDefaultStorySettings() still overrides these at boot, but this
  // static fallback keeps the app on the custom path even with no .env files.
  textProvider: "custom",
  localTextModel: DEFAULT_LOCAL_TEXT_MODEL,
  customBaseUrl: "http://127.0.0.1:8080/v1",
  customModel: "gemma-4-12b-uncensored",
  customApiKey: "",
  imageMode: "fast",
  imageBackend: "sdnq-hs",
  aspect: "square",
  imageGenerationEnabled: true,
  autoImages: true,
  rpgEnabled: false,
  proseSize: "medium",
  responseLength: "medium",
  voice: "RU_Male_Gabidullin_ruslan",
  autoplay: false,
  ttsVolume: 1,
  ttsSpeed: 1,
};

export function titleFromInput(input: string) {
  const compact = input.replace(/\s+/g, " ").trim();
  if (!compact) {
    return DEFAULT_CHAT_TITLE;
  }

  return compact.length > 58 ? `${compact.slice(0, 55).trim()}...` : compact;
}
