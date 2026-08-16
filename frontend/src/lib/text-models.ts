// "custom" = any OpenAI-compatible server (llama.cpp, LM Studio, vLLM,
// OpenRouter, a remote Ollama). OpenRouter is no longer its own provider; it
// is reached through "custom" with the OpenRouter preset URL.
export type TextProvider = "local" | "custom";

export const TEXT_PROVIDERS: TextProvider[] = ["local", "custom"];

// Gemma 4 QAT (quantization-aware trained, Q4_0) builds served by Ollama.
// context = native context window in tokens. ram = resident memory measured
// via `ollama ps` at the app's context settings (M2 Max; the E-models stream
// per-layer embeddings, so they sit below their download size).
export const LOCAL_TEXT_MODELS = [
  { id: "gemma4:e2b-it-qat", label: "Gemma 4 E2B", size: "4.3 GB", ram: "4.5 GB", context: 131_072 },
  { id: "gemma4:e4b-it-qat", label: "Gemma 4 E4B", size: "6.1 GB", ram: "3.2 GB", context: 131_072 },
  { id: "gemma4:12b-it-qat", label: "Gemma 4 12B", size: "7.2 GB", ram: "7.7 GB", context: 262_144 },
  { id: "gemma4:26b-a4b-it-qat", label: "Gemma 4 26B MoE", size: "16 GB", ram: "15 GB", context: 262_144 },
  { id: "gemma4:31b-it-qat", label: "Gemma 4 31B", size: "19 GB", ram: "~20 GB", context: 262_144 },
] as const;

export function localModelContextWindow(modelId: string): number {
  return LOCAL_TEXT_MODELS.find((model) => model.id === modelId)?.context ?? 131_072;
}

export type LocalTextModelId = (typeof LOCAL_TEXT_MODELS)[number]["id"];

export const LOCAL_TEXT_MODEL_IDS = LOCAL_TEXT_MODELS.map(
  (model) => model.id,
) as [LocalTextModelId, ...LocalTextModelId[]];

export const DEFAULT_LOCAL_TEXT_MODEL: LocalTextModelId = "gemma4:12b-it-qat";

export function isLocalTextModelId(value: unknown): value is LocalTextModelId {
  return LOCAL_TEXT_MODEL_IDS.includes(value as LocalTextModelId);
}

export function isTextProvider(value: unknown): value is TextProvider {
  return value === "local" || value === "custom";
}
