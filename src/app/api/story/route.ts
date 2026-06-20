import { z } from "zod";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  addEvents,
  addItems,
  addMessage,
  getCharacterRpg,
  getCharacterRpgMap,
  getCombatants,
  getStorySummary,
  listCharacters,
  listItems,
  saveCharacterRpg,
  setCombatants,
  setStorySummary,
  updateChatTitleFromInput,
} from "@/lib/db";
import { applyGameUpdate, type ActorMap } from "@/lib/rpg/apply";
import { extractGameUpdate } from "@/lib/rpg/parse";
import { buildRpgSection } from "@/lib/rpg/prompt";
import type { Enemy, GameEvent } from "@/lib/rpg/types";
import { serverEnv } from "@/lib/server-env";
import {
  buildStoryMessages,
  extractStoryText,
  finalizeScenePrompt,
  packStoryHistory,
} from "@/lib/story-prompt";
import {
  DEFAULT_LOCAL_TEXT_MODEL,
  LOCAL_TEXT_MODEL_IDS,
  localModelContextWindow,
} from "@/lib/text-models";
import { PROSE_SIZE_VALUES, RESPONSE_LENGTH_VALUES } from "@/lib/types";
import type { Attachment, StoryCharacter, StoryMessage } from "@/lib/types";

export const runtime = "nodejs";

const MAX_IMAGE_REFERENCES = 2;
const DEFAULT_MAX_OUTPUT_TOKENS = 16_384;
const MAX_CONFIGURABLE_OUTPUT_TOKENS = 65_536;
const DEFAULT_LOCAL_MAX_OUTPUT_TOKENS = 4_096;
const DEFAULT_CUSTOM_TEXT_TIMEOUT_MS = 3 * 60 * 1000;
const DEFAULT_LOCAL_TEXT_TIMEOUT_MS = 6 * 60 * 1000;
const MIN_TEXT_TIMEOUT_MS = 30 * 1000;
const MAX_TEXT_TIMEOUT_MS = 30 * 60 * 1000;
const WINDOWS_DEFAULT_LOCAL_CONTEXT_TOKENS = 65_536;
// Rough chars-per-token for English prose, used to budget story history.
const HISTORY_CHARS_PER_TOKEN = 3.6;
// Tokens held back for the system prompt, character portraits, and the reply.
const HISTORY_RESERVE_TOKENS = 8_192;
// Stay comfortably under the context window so a turn can never max it out.
const CONTEXT_SAFETY_MARGIN = 0.9;
const MIN_HISTORY_CHAR_BUDGET = 48_000;
// History budget for remote/custom backends, whose context window we can't
// introspect. ~43K tokens; long stories still compact via the rolling summary.
const REMOTE_HISTORY_CHAR_BUDGET = 172_800;
const supportedVisionTypes = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);

type TextContentPart = {
  type: "text";
  text: string;
};

type ImageContentPart = {
  type: "image_url";
  image_url: {
    url: string;
  };
};

type OpenRouterMessage = {
  role: "system" | "user" | "assistant";
  content: string | Array<TextContentPart | ImageContentPart>;
};

const attachmentSchema = z.object({
  id: z.string(),
  name: z.string(),
  type: z.string(),
  url: z.string(),
  dataUrl: z.string().optional(),
});

const messageSchema = z.object({
  id: z.string(),
  role: z.enum(["user", "assistant"]),
  content: z.string(),
  createdAt: z.string(),
  attachments: z.array(attachmentSchema).optional(),
});

const requestSchema = z.object({
  chatId: z.string().optional(),
  userMessageId: z.string().optional(),
  // turn     — a normal player action; the input is persisted as a user message.
  // kickoff  — write the opening passage from a directive (not persisted).
  // continue — advance the story with no player action (not persisted).
  // retry    — regenerate the latest passage; input is the prior player action,
  //            already saved, so it is not persisted again.
  // opening — the player wrote the first passage themselves; store it verbatim
  //           as the opening narration, with no model call.
  mode: z.enum(["turn", "kickoff", "continue", "retry", "opening"]).default("turn"),
  input: z.string().min(1),
  messages: z.array(messageSchema).default([]),
  attachments: z.array(attachmentSchema).default([]),
  settings: z.object({
    world: z.string().default(""),
    style: z.string().default(""),
    narratorPrompt: z.string().default(""),
    imagePrompt: z.string().default(""),
    imageStylePrefix: z.string().default(""),
    antiRepetition: z.boolean().default(true),
    causeAwareEnding: z.boolean().default(true),
    multiVoice: z.boolean().default(false),
    companion: z.boolean().default(false),
    textProvider: z.enum(["local", "custom"]).default("custom"),
    localTextModel: z.enum(LOCAL_TEXT_MODEL_IDS).default(DEFAULT_LOCAL_TEXT_MODEL),
    customBaseUrl: z.string().trim().max(500).default(""),
    customModel: z.string().trim().max(200).default(""),
    customApiKey: z.string().trim().max(400).default(""),
    imageMode: z.enum(["fast", "slow"]).default("slow"),
    imageBackend: z.enum(["mflux-hs", "sdnq-hs", "flux-uncensored"]).default("mflux-hs"),
    aspect: z.enum(["square", "portrait", "landscape"]).default("square"),
    imageGenerationEnabled: z.boolean().default(true),
    autoImages: z.boolean().default(true),
    rpgEnabled: z.boolean().default(false),
    diceEnabled: z.boolean().default(true),
    diceSound: z.boolean().default(true),
    diceVolume: z.number().default(55),
    proseSize: z.enum(PROSE_SIZE_VALUES).default("medium"),
    responseLength: z.enum(RESPONSE_LENGTH_VALUES).default("medium"),
    voice: z.string().default("RU_Male_Gabidullin_ruslan"),
    autoplay: z.boolean().default(false),
    ttsVolume: z.number().default(1),
    ttsSpeed: z.number().default(1),
  }),
});

const generateImageTool = {
  type: "function",
  function: {
    name: "generate_image",
    description:
      "Request one local FLUX image for a meaningful visual beat in the current roleplay scene. Use sparingly.",
    parameters: {
      type: "object",
      additionalProperties: false,
      properties: {
        prompt: {
          type: "string",
          description:
            "Detailed visual prompt. Include subject, environment, composition, lighting, camera style, mood, and avoid text overlays. For established characters, describe visible physical features and whether each person is a man or woman; do not rely on character names inside the prompt. Write this prompt in English.",
        },
        reason: {
          type: "string",
          description: "Short private reason this scene benefits from an image.",
        },
        characterIds: {
          type: "array",
          maxItems: MAX_IMAGE_REFERENCES,
          items: { type: "string" },
          description:
            "Exact saved character IDs to pass as visual references. Use at most two, and only when those characters should appear.",
        },
      },
      required: ["prompt"],
    },
  },
} as const;

const imageToolArgsSchema = z.object({
  prompt: z.string().min(1),
  reason: z.string().optional(),
  characterIds: z.array(z.string()).max(MAX_IMAGE_REFERENCES).optional(),
});

function mimeFromAttachment(attachment: Attachment) {
  if (supportedVisionTypes.has(attachment.type)) {
    return attachment.type;
  }

  const extension = attachment.url.split(".").pop()?.toLowerCase();
  if (extension === "png") {
    return "image/png";
  }
  if (extension === "jpg" || extension === "jpeg") {
    return "image/jpeg";
  }
  if (extension === "webp") {
    return "image/webp";
  }
  if (extension === "gif") {
    return "image/gif";
  }

  return null;
}

function localImageDataUrl(attachment: Attachment) {
  if (attachment.dataUrl?.startsWith("data:image/")) {
    return attachment.dataUrl;
  }

  const mime = mimeFromAttachment(attachment);
  if (!mime || !attachment.url.startsWith("/")) {
    return null;
  }

  const publicDir = path.join(process.cwd(), "public");
  const localPath = path.resolve(publicDir, attachment.url.replace(/^\/+/, ""));

  if (!localPath.startsWith(`${publicDir}${path.sep}`) || !existsSync(localPath)) {
    return null;
  }

  const encoded = readFileSync(localPath).toString("base64");
  return `data:${mime};base64,${encoded}`;
}

function buildCharacterVisionMessage(characters: StoryCharacter[]): OpenRouterMessage | null {
  const parts: Array<TextContentPart | ImageContentPart> = [
    {
      type: "text",
      text:
        "Сохранённые ссылки на портреты персонажей для визуальной согласованности. Каждый портрет подписан именем персонажа и точным ID. Используй эти изображения, чтобы понять, как выглядят персонажи, и используй точные ID при вызове generate_image.characterIds.",
    },
  ];

  let attachedCount = 0;
  for (const character of characters) {
    if (!character.portrait) {
      continue;
    }

    const dataUrl = localImageDataUrl(character.portrait);
    if (!dataUrl) {
      continue;
    }

    attachedCount += 1;
    parts.push({
      type: "text",
      text: [
        `Портрет персонажа ${attachedCount}: ${character.name}`,
        `ID: ${character.id}`,
        character.details ? `Детали: ${character.details}` : "",
        `Следующее изображение — это ${character.name}.`,
      ]
        .filter(Boolean)
        .join("\n"),
    });
    parts.push({
      type: "image_url",
      image_url: {
        url: dataUrl,
      },
    });
  }

  if (attachedCount === 0) {
    return null;
  }

  return {
    role: "user",
    content: parts,
  };
}

function parseGenerateImageToolCall(toolCalls: unknown) {
  if (!Array.isArray(toolCalls)) {
    return null;
  }

  for (const call of toolCalls) {
    if (!call || typeof call !== "object" || !("function" in call)) {
      continue;
    }

    const fn = call.function;
    if (!fn || typeof fn !== "object" || !("name" in fn) || fn.name !== "generate_image") {
      continue;
    }

    const rawArguments = "arguments" in fn ? fn.arguments : undefined;

    let parsed: unknown = rawArguments;
    if (typeof rawArguments === "string") {
      try {
        parsed = JSON.parse(rawArguments) as unknown;
      } catch {
        return null;
      }
    } else if (!rawArguments) {
      parsed = {};
    }

    const result = imageToolArgsSchema.safeParse(parsed);
    return result.success ? result.data : null;
  }

  return null;
}

function configuredMaxOutputTokens() {
  const raw = serverEnv("OPENROUTER_MAX_TOKENS");
  const parsed = Number.parseInt(raw, 10);

  if (!Number.isFinite(parsed)) {
    return DEFAULT_MAX_OUTPUT_TOKENS;
  }

  return Math.max(512, Math.min(parsed, MAX_CONFIGURABLE_OUTPUT_TOKENS));
}

function localMaxOutputTokens() {
  const parsed = Number.parseInt(serverEnv("LOCAL_TEXT_MAX_TOKENS"), 10);

  if (!Number.isFinite(parsed)) {
    return DEFAULT_LOCAL_MAX_OUTPUT_TOKENS;
  }

  return Math.max(256, Math.min(parsed, MAX_CONFIGURABLE_OUTPUT_TOKENS));
}

// Defaults to the model's full native window: Gemma 4's sliding-window
// attention keeps the KV cache small even at 256K, so memory is not the
// limiting factor. LOCAL_TEXT_CONTEXT can cap it to bound worst-case
// prefill time on very long stories.
function localContextTokens(model: string) {
  const native = localModelContextWindow(model);
  const raw = serverEnv("LOCAL_TEXT_CONTEXT");
  const parsed = Number.parseInt(raw, 10);

  if (!Number.isFinite(parsed)) {
    return process.platform === "win32"
      ? Math.min(native, WINDOWS_DEFAULT_LOCAL_CONTEXT_TOKENS)
      : native;
  }

  return Math.max(2_048, Math.min(parsed, native));
}

function configuredTextTimeoutMs(envKey: string, fallback: number) {
  const parsed = Number.parseInt(serverEnv(envKey), 10);

  if (!Number.isFinite(parsed)) {
    return fallback;
  }

  return Math.max(MIN_TEXT_TIMEOUT_MS, Math.min(parsed, MAX_TEXT_TIMEOUT_MS));
}

function formatTimeout(ms: number) {
  const seconds = Math.round(ms / 1000);
  if (seconds >= 60) {
    const minutes = Math.round(seconds / 60);
    return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
  }

  return `${seconds} seconds`;
}

function createRequestTimeout(ms: number) {
  const controller = new AbortController();
  let timedOut = false;
  const timeoutId = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, ms);

  return {
    signal: controller.signal,
    clear: () => clearTimeout(timeoutId),
    timedOut: () => timedOut,
  };
}

type UpstreamChatMessage = {
  content?: unknown;
  tool_calls?: unknown;
};

type UpstreamResult = {
  message?: UpstreamChatMessage;
  error?: Response;
};

type OllamaChatMessage = {
  role: "system" | "user" | "assistant";
  content: string;
  images?: string[];
};

function toOllamaMessages(messages: OpenRouterMessage[]): OllamaChatMessage[] {
  return messages.map((message) => {
    if (typeof message.content === "string") {
      return { role: message.role, content: message.content };
    }

    const textParts: string[] = [];
    const images: string[] = [];
    for (const part of message.content) {
      if (part.type === "text") {
        textParts.push(part.text);
        continue;
      }

      // Only data URLs reach this point; Ollama wants the bare base64 payload.
      const base64 = part.image_url.url.split(",")[1];
      if (base64) {
        images.push(base64);
      }
    }

    return {
      role: message.role,
      content: textParts.join("\n\n"),
      ...(images.length ? { images } : {}),
    };
  });
}

// Resolve a user-entered backend URL to its /chat/completions endpoint.
// Accepts a bare host (http://127.0.0.1:8080), a versioned base (.../v1), or
// the full endpoint, so people can paste whatever their server prints.
function customChatEndpoint(baseUrl: string): string {
  const url = baseUrl.trim().replace(/\/+$/, "");
  if (/\/chat\/completions$/.test(url)) return url;
  if (/\/v\d+$/.test(url)) return `${url}/chat/completions`;
  return `${url}/v1/chat/completions`;
}

// Any OpenAI-compatible server: llama.cpp, LM Studio, vLLM, TabbyAPI,
// KoboldCpp, OpenRouter, a remote Ollama, etc. The model name and base URL are
// per-chat settings; the key is optional (most local servers need none). When
// the URL is OpenRouter we add its attribution headers and fall back to the
// OPENROUTER_* env vars; otherwise the fallback is OPENAI_COMPAT_API_KEY.
async function requestCustomMessage(
  baseUrl: string,
  model: string,
  apiKey: string,
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
): Promise<UpstreamResult> {
  const trimmedBase = (baseUrl || "").trim();

  if (!trimmedBase) {
    return {
      error: Response.json(
        {
          error:
            "Не указан URL сервера. Добавьте URL вашего сервера (например http://127.0.0.1:8080/v1) в настройки текстовой модели.",
        },
        { status: 400 },
      ),
    };
  }

  const isOpenRouter = /(^|\.)openrouter\.ai/i.test(trimmedBase);
  const resolvedModel =
    (model || "").trim() ||
    serverEnv("OPENAI_COMPAT_MODEL") ||
    (isOpenRouter ? serverEnv("OPENROUTER_MODEL", "google/gemini-3.5-flash") : "");

  if (!resolvedModel) {
    return {
      error: Response.json(
        {
          error:
            "Не указано имя модели. Введите модель, которую обслуживает ваш сервер, в настройки текстовой модели.",
        },
        { status: 400 },
      ),
    };
  }

  const endpoint = customChatEndpoint(trimmedBase);
  // In-app key wins; otherwise fall back to the matching env var.
  const resolvedKey =
    (apiKey || "").trim() ||
    (isOpenRouter ? serverEnv("OPENROUTER_API_KEY") : "") ||
    serverEnv("OPENAI_COMPAT_API_KEY");
  const requestPayload: Record<string, unknown> = {
    model: resolvedModel,
    messages,
    temperature: 0.9,
    max_tokens: configuredMaxOutputTokens(),
  };

  if (includeImageTool) {
    requestPayload.tools = [generateImageTool];
    requestPayload.tool_choice = "auto";
  }

  const timeoutMs = configuredTextTimeoutMs(
    "CUSTOM_TEXT_TIMEOUT_MS",
    DEFAULT_CUSTOM_TEXT_TIMEOUT_MS,
  );
  const requestTimeout = createRequestTimeout(timeoutMs);
  let upstream: Response;
  try {
    upstream = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(resolvedKey ? { Authorization: `Bearer ${resolvedKey}` } : {}),
        ...(isOpenRouter
          ? {
              "HTTP-Referer": serverEnv("OPENROUTER_APP_URL", "http://localhost:3000"),
              "X-Title": serverEnv("OPENROUTER_APP_TITLE", "Open Dungeon"),
            }
          : {}),
      },
      body: JSON.stringify(requestPayload),
      signal: requestTimeout.signal,
    });
  } catch {
    if (requestTimeout.timedOut()) {
      return {
        error: Response.json(
          {
            error: `${isOpenRouter ? "OpenRouter" : "Backend"} request timed out after ${formatTimeout(timeoutMs)}. The server may still be generating in the background; wait a moment, then retry or lower that backend's context/output settings.`,
          },
          { status: 504 },
        ),
      };
    }

    return {
      error: Response.json(
        {
          error: `Could not reach the backend at ${endpoint}. Check the URL and that your server is running.`,
        },
        { status: 502 },
      ),
    };
  } finally {
    requestTimeout.clear();
  }

  if (!upstream.ok) {
    const text = await upstream.text();

    // Some servers don't implement function tools; retry without auto images.
    if (includeImageTool && /tool|function|not support/i.test(text)) {
      return requestCustomMessage(trimmedBase, resolvedModel, apiKey, messages, false);
    }

    return {
      error: Response.json(
        {
          error: `${isOpenRouter ? "OpenRouter" : "Backend"} request failed (${upstream.status}).`,
          detail: text.slice(0, 1000),
        },
        { status: upstream.status },
      ),
    };
  }

  const data = (await upstream.json()) as {
    choices?: Array<{ message?: UpstreamChatMessage }>;
  };

  return { message: data?.choices?.[0]?.message };
}

// A streamed delta from the upstream, normalised across OpenAI-compatible
// servers: visible story text fragments, plus incremental generate_image
// tool-call argument fragments keyed by their tool-call index.
type StreamEvent =
  | { type: "text"; text: string }
  | { type: "tool"; index: number; name?: string; argsFragment?: string }
  | { type: "done" };

type StreamStart =
  | { stream: AsyncGenerator<StreamEvent>; error?: undefined }
  | { stream?: undefined; error: Response };

// Streaming twin of requestCustomMessage: same endpoint/model/key/header
// resolution, but asks for stream:true and parses the SSE body into
// StreamEvents. The route forwards text fragments to the client as they
// arrive while accumulating the full passage + tool call server-side.
async function requestCustomMessageStream(
  baseUrl: string,
  model: string,
  apiKey: string,
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
): Promise<StreamStart> {
  const trimmedBase = (baseUrl || "").trim();

  if (!trimmedBase) {
    return {
      error: Response.json(
        {
          error:
            "Не указан URL сервера. Добавьте URL вашего сервера (например http://127.0.0.1:8080/v1) в настройки текстовой модели.",
        },
        { status: 400 },
      ),
    };
  }

  const isOpenRouter = /(^|\.)openrouter\.ai/i.test(trimmedBase);
  const resolvedModel =
    (model || "").trim() ||
    serverEnv("OPENAI_COMPAT_MODEL") ||
    (isOpenRouter ? serverEnv("OPENROUTER_MODEL", "google/gemini-3.5-flash") : "");

  if (!resolvedModel) {
    return {
      error: Response.json(
        {
          error:
            "Не указано имя модели. Введите модель, которую обслуживает ваш сервер, в настройки текстовой модели.",
        },
        { status: 400 },
      ),
    };
  }

  const endpoint = customChatEndpoint(trimmedBase);
  const resolvedKey =
    (apiKey || "").trim() ||
    (isOpenRouter ? serverEnv("OPENROUTER_API_KEY") : "") ||
    serverEnv("OPENAI_COMPAT_API_KEY");
  const requestPayload: Record<string, unknown> = {
    model: resolvedModel,
    messages,
    temperature: 0.9,
    max_tokens: configuredMaxOutputTokens(),
    stream: true,
  };

  if (includeImageTool) {
    requestPayload.tools = [generateImageTool];
    requestPayload.tool_choice = "auto";
  }

  const timeoutMs = configuredTextTimeoutMs(
    "CUSTOM_TEXT_TIMEOUT_MS",
    DEFAULT_CUSTOM_TEXT_TIMEOUT_MS,
  );
  const requestTimeout = createRequestTimeout(timeoutMs);
  let upstream: Response;
  try {
    upstream = await fetch(endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "text/event-stream",
        ...(resolvedKey ? { Authorization: `Bearer ${resolvedKey}` } : {}),
        ...(isOpenRouter
          ? {
              "HTTP-Referer": serverEnv("OPENROUTER_APP_URL", "http://localhost:3000"),
              "X-Title": serverEnv("OPENROUTER_APP_TITLE", "Open Dungeon"),
            }
          : {}),
      },
      body: JSON.stringify(requestPayload),
      signal: requestTimeout.signal,
    });
  } catch {
    requestTimeout.clear();
    if (requestTimeout.timedOut()) {
      return {
        error: Response.json(
          {
            error: `${isOpenRouter ? "OpenRouter" : "Backend"} request timed out after ${formatTimeout(timeoutMs)}. The server may still be generating in the background; wait a moment, then retry or lower that backend's context/output settings.`,
          },
          { status: 504 },
        ),
      };
    }

    return {
      error: Response.json(
        {
          error: `Could not reach the backend at ${endpoint}. Check the URL and that your server is running.`,
        },
        { status: 502 },
      ),
    };
  }

  if (!upstream.ok || !upstream.body) {
    const text = upstream.body ? await upstream.text() : "";
    requestTimeout.clear();

    // Some servers reject function tools; retry the stream without auto images.
    if (includeImageTool && /tool|function|not support/i.test(text)) {
      return requestCustomMessageStream(trimmedBase, resolvedModel, apiKey, messages, false);
    }

    return {
      error: Response.json(
        {
          error: `${isOpenRouter ? "OpenRouter" : "Backend"} request failed (${upstream.status}).`,
          detail: text.slice(0, 1000),
        },
        { status: upstream.status },
      ),
    };
  }

  const body = upstream.body;

  async function* parseSse(): AsyncGenerator<StreamEvent> {
    const reader = body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
      while (true) {
        const { value, done } = await reader.read();
        if (done) {
          break;
        }
        buffer += decoder.decode(value, { stream: true });

        // SSE events are separated by a blank line; a data: line carries one
        // JSON chunk (or [DONE]). Split on newlines and process whole lines.
        let newlineIndex = buffer.indexOf("\n");
        while (newlineIndex !== -1) {
          const line = buffer.slice(0, newlineIndex).replace(/\r$/, "").trim();
          buffer = buffer.slice(newlineIndex + 1);
          newlineIndex = buffer.indexOf("\n");

          if (!line || line.startsWith(":")) {
            continue;
          }
          if (!line.startsWith("data:")) {
            continue;
          }
          const payload = line.slice(5).trim();
          if (payload === "[DONE]") {
            yield { type: "done" };
            return;
          }

          let chunk: {
            choices?: Array<{
              delta?: {
                content?: unknown;
                tool_calls?: Array<{
                  index?: number;
                  function?: { name?: string; arguments?: string };
                }>;
              };
              finish_reason?: string | null;
            }>;
          };
          try {
            chunk = JSON.parse(payload);
          } catch {
            continue;
          }

          const delta = chunk.choices?.[0]?.delta;
          if (!delta) {
            continue;
          }
          if (typeof delta.content === "string" && delta.content) {
            yield { type: "text", text: delta.content };
          } else if (Array.isArray(delta.content)) {
            const text = extractStoryText(delta.content);
            if (text) {
              yield { type: "text", text };
            }
          }
          if (Array.isArray(delta.tool_calls)) {
            for (const call of delta.tool_calls) {
              yield {
                type: "tool",
                index: typeof call.index === "number" ? call.index : 0,
                name: call.function?.name,
                argsFragment: call.function?.arguments,
              };
            }
          }
        }
      }
      yield { type: "done" };
    } finally {
      requestTimeout.clear();
      reader.releaseLock();
    }
  }

  return { stream: parseSse() };
}

async function requestLocalMessage(
  model: string,
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
  disableThinking = true,
): Promise<UpstreamResult> {
  // Optional, secondary path. Only reached when a chat explicitly selects the
  // "Ollama" (local) provider; the default text path is the custom server.
  const baseUrl = serverEnv("OLLAMA_BASE_URL", "http://127.0.0.1:11434").replace(/\/$/, "");
  const requestPayload: Record<string, unknown> = {
    model,
    messages: toOllamaMessages(messages),
    stream: false,
    // Keep the model (and the story's prompt cache) resident between turns.
    keep_alive: "30m",
    options: {
      temperature: 0.9,
      num_predict: localMaxOutputTokens(),
      num_ctx: localContextTokens(model),
    },
  };

  // Gemma 4 is a hybrid reasoning model; without this it spends most of the
  // token budget on a hidden "thinking" channel before any story text.
  if (disableThinking) {
    requestPayload.think = false;
  }

  if (includeImageTool) {
    requestPayload.tools = [generateImageTool];
  }

  const timeoutMs = configuredTextTimeoutMs("LOCAL_TEXT_TIMEOUT_MS", DEFAULT_LOCAL_TEXT_TIMEOUT_MS);
  const requestTimeout = createRequestTimeout(timeoutMs);
  let upstream: Response;
  try {
    upstream = await fetch(`${baseUrl}/api/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(requestPayload),
      signal: requestTimeout.signal,
    });
  } catch {
    if (requestTimeout.timedOut()) {
      return {
        error: Response.json(
          {
            error: `The local model took longer than ${formatTimeout(timeoutMs)} to answer. Ollama may still be working in the background; wait a moment, then retry, restart Ollama if your fans stay high, or lower LOCAL_TEXT_CONTEXT / LOCAL_TEXT_MAX_TOKENS.`,
          },
          { status: 504 },
        ),
      };
    }

    return {
      error: Response.json(
        {
          error: `Could not reach Ollama at ${baseUrl}. Start the Ollama app (or \`ollama serve\`), pull the model with \`ollama pull ${model}\`, or switch this chat to OpenRouter in Text Model settings.`,
        },
        { status: 502 },
      ),
    };
  } finally {
    requestTimeout.clear();
  }

  if (!upstream.ok) {
    const text = await upstream.text();

    // Some local models lack a tool-call template; retry the turn without auto images.
    if (includeImageTool && /does not support tools/i.test(text)) {
      return requestLocalMessage(model, messages, false, disableThinking);
    }

    // Models without a thinking channel reject the think parameter; retry without it.
    if (disableThinking && /does not support think/i.test(text)) {
      return requestLocalMessage(model, messages, includeImageTool, false);
    }

    const hint = /not found/i.test(text)
      ? ` The model is not installed — run \`ollama pull ${model}\`.`
      : "";
    return {
      error: Response.json(
        {
          error: `Local model request failed (${upstream.status}).${hint}`,
          detail: text.slice(0, 1000),
        },
        { status: 502 },
      ),
    };
  }

  const data = (await upstream.json()) as { message?: UpstreamChatMessage };
  return { message: data?.message };
}

const SUMMARIZER_SYSTEM = `Ты сохраняешь каноническую память «истории до сих пор» для текущей интерактивной ролевой игры. Объедини существующее резюме с новыми отрывками в одно обновленное резюме.

Сохраняй с приоритетом: активные сюжетные линии и их текущее состояние; персонажи (имена, роли, отношения, отличительные физические черты); обещания, долги, секреты, ранения и предметы, которые могут иметь значение позже; локации и порядок основных событий; выборы, которые игрок сделал и которые сформировали историю.

Пиши компактную прозу в прошедшем времени, без заголовков или списков, максимум 500 слов. Выводи только обновленное резюме.`;

type StoryRequestSettings = z.infer<typeof requestSchema>["settings"];

// Single place that picks the upstream provider for a turn.
function requestStoryMessage(
  settings: StoryRequestSettings,
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
): Promise<UpstreamResult> {
  if (settings.textProvider === "local") {
    return requestLocalMessage(settings.localTextModel, messages, includeImageTool);
  }
  return requestCustomMessage(
    settings.customBaseUrl,
    settings.customModel,
    settings.customApiKey,
    messages,
    includeImageTool,
  );
}

// Codex-style compaction adapted for stories: passages that scroll out of the
// context window are folded into a rolling summary instead of being forgotten.
// Best-effort — a failed summary never blocks the player's turn.
async function summarizeEvictedPassages(
  settings: StoryRequestSettings,
  existingSummary: string,
  passages: Array<{ role: "user" | "assistant"; content: string }>,
): Promise<string | null> {
  const transcript = passages
    .map((message) => `${message.role === "user" ? "Игрок" : "Рассказчик"}: ${message.content}`)
    .join("\n\n");
  const messages: OpenRouterMessage[] = [
    { role: "system", content: SUMMARIZER_SYSTEM },
    {
      role: "user",
      content: `Существующее резюме:\n${existingSummary || "(ещё нет)"}\n\nНовые отрывки для включения:\n${transcript}`,
    },
  ];

  const { message, error } = await requestStoryMessage(settings, messages, false);

  if (error) {
    return null;
  }

  const summary = extractStoryText(message?.content);
  return summary ? summary.slice(0, 8_000) : null;
}

// Resolve the narrator's [[GAME]] block (when RPG is on): roll dice server-side,
// apply HP/death, persist character state + journal events, and return the prose
// with the block stripped plus the events to surface to the client.
function resolveRpgTurn(
  chatId: string | undefined,
  enabled: boolean,
  characterActors: ActorMap,
  enemies: Enemy[],
  storyText: string,
): { clean: string; events: GameEvent[] } {
  if (!enabled) {
    return { clean: storyText, events: [] };
  }
  const { clean, update } = extractGameUpdate(storyText);
  if (!update) {
    return { clean, events: [] };
  }
  // Combined actor map: player characters + current enemies, so the engine can
  // resolve attacks/HP against either side and target newly-spawned foes.
  const actors: ActorMap = new Map(characterActors);
  const enemyIds = new Set(enemies.map((enemy) => enemy.id));
  for (const enemy of enemies) actors.set(enemy.id, { name: enemy.name, rpg: enemy.rpg });

  const { events, changed, items, spawnedEnemies } = applyGameUpdate(update, actors);
  if (chatId) {
    for (const id of changed) {
      if (enemyIds.has(id)) continue; // enemies are persisted together below
      const actor = actors.get(id);
      if (actor && characterActors.has(id)) {
        // actor.rpg is the DERIVED block (base + equipped modifiers). Persist
        // ONLY the mutated HP and dead flag back onto the canonical BASE row, so
        // equipment bonuses are never baked into stored base stats (which would
        // then compound every turn as gear is re-folded on top).
        const base = getCharacterRpg(chatId, id);
        if (base) {
          // Persist the post-turn current HP at the DERIVED cap (incl. any +maxHp
          // gear) so HP healed into that headroom isn't lost. The stored base row may
          // then hold current > base.hp.max while the gear is worn — that's fine: every
          // reader re-derives and re-clamps (deriveRpg / the HUD), and unequipping folds
          // current back down to base max on the next derive.
          base.hp.current = Math.min(actor.rpg.hp.current, actor.rpg.hp.max);
          base.dead = actor.rpg.dead;
          saveCharacterRpg(id, base);
        }
      }
    }
    // Rebuild the encounter: surviving enemies (mutated in place) + new spawns,
    // dropping the defeated so they don't linger into the next turn.
    const nextEnemies = [...enemies, ...spawnedEnemies].filter((enemy) => !enemy.rpg.dead);
    setCombatants(chatId, nextEnemies);
    addEvents(chatId, events);
    addItems(chatId, items);
  }
  return { clean, events };
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const characters = body.chatId ? listCharacters(body.chatId) : [];
  const knownCharacterIds = new Set(characters.map((character) => character.id));
  // The protagonist = the first character created (oldest row). listCharacters
  // orders by updated_at DESC, so sort a copy by createdAt to find them.
  const heroId =
    [...characters].sort((a, b) => a.createdAt.localeCompare(b.createdAt))[0]?.id;
  // Ensure the player character rides along as a visual reference whenever a
  // scene is illustrated, so the evolving-hero image2image reference attaches
  // and refreshes even if the narrator didn't name them. Keeps within the
  // reference cap and only adds a known id once.
  const withHeroReference = (ids: string[]): string[] => {
    if (!heroId || ids.includes(heroId) || ids.length >= MAX_IMAGE_REFERENCES) {
      return ids;
    }
    return [heroId, ...ids].slice(0, MAX_IMAGE_REFERENCES);
  };
  const rpgEnabled = body.settings.rpgEnabled;
  const rpgActors: ActorMap =
    rpgEnabled && body.chatId ? getCharacterRpgMap(body.chatId) : new Map();
  const rpgEnemies: Enemy[] = rpgEnabled && body.chatId ? getCombatants(body.chatId) : [];
  const rpgSection = rpgEnabled
    ? buildRpgSection(rpgActors, body.chatId ? listItems(body.chatId) : [], rpgEnemies)
    : "";
  const userMessage: StoryMessage = {
    id: body.userMessageId || crypto.randomUUID(),
    role: "user",
    content: body.input,
    createdAt: new Date().toISOString(),
    attachments: body.attachments,
  };

  if (body.chatId && body.mode === "turn") {
    addMessage(body.chatId, userMessage);
    updateChatTitleFromInput(body.chatId, body.input);
  }

  // Player-authored opening: persist the text as the first narration passage
  // and return it, with no model call.
  if (body.mode === "opening") {
    const openingMessage: StoryMessage = {
      id: crypto.randomUUID(),
      role: "assistant",
      content: body.input,
      createdAt: new Date().toISOString(),
    };
    if (body.chatId) {
      addMessage(body.chatId, openingMessage);
    }
    return Response.json({ id: openingMessage.id, content: openingMessage.content });
  }

  const provider = body.settings.textProvider;
  const historyCharBudget =
    provider === "local"
      ? Math.max(
          MIN_HISTORY_CHAR_BUDGET,
          Math.round(
            (localContextTokens(body.settings.localTextModel) - HISTORY_RESERVE_TOKENS) *
              CONTEXT_SAFETY_MARGIN *
              HISTORY_CHARS_PER_TOKEN,
          ),
        )
      : REMOTE_HISTORY_CHAR_BUDGET;

  const { recent, evicted } = packStoryHistory(body.messages, historyCharBudget);
  let storySummary = "";

  if (body.chatId) {
    const stored = getStorySummary(body.chatId);
    storySummary = stored.summary;

    if (evicted.length > stored.coveredCount) {
      const folded = await summarizeEvictedPassages(
        body.settings,
        stored.summary,
        evicted.slice(stored.coveredCount),
      );
      if (folded) {
        storySummary = folded;
        setStorySummary(body.chatId, folded, evicted.length);
      }
    }
  }

  const storyMessages = buildStoryMessages(
    [
      ...recent,
      ...(body.attachments.length
        ? [
            {
              id: "pending-attachments",
              role: "user" as const,
              content: "Игрок включил визуальные ссылки для этого хода.",
              createdAt: new Date().toISOString(),
              attachments: body.attachments,
            },
          ]
        : []),
    ],
    body.input,
    body.settings,
    characters,
    storySummary,
    rpgSection,
  ) as OpenRouterMessage[];
  const characterVisionMessage = buildCharacterVisionMessage(characters);
  const messages = characterVisionMessage
    ? [storyMessages[0], characterVisionMessage, ...storyMessages.slice(1)]
    : storyMessages;
  const includeImageTool = body.settings.imageGenerationEnabled && body.settings.autoImages;

  // Streaming path: only the custom OpenAI-compatible backend advertises SSE.
  // The Ollama ("local") provider stays on the buffered path below.
  if (provider === "custom") {
    const { stream, error: streamError } = await requestCustomMessageStream(
      body.settings.customBaseUrl,
      body.settings.customModel,
      body.settings.customApiKey,
      messages,
      includeImageTool,
    );

    if (streamError) {
      return streamError;
    }

    const encoder = new TextEncoder();
    const sse = (event: string, data: unknown) =>
      encoder.encode(`event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
    const chatId = body.chatId;

    const responseStream = new ReadableStream<Uint8Array>({
      async start(controller) {
        let storyText = "";
        let flushedLen = 0; // chars of storyText already streamed to the client
        const toolArgsByIndex = new Map<number, string>();
        let sawImageTool = false;

        // The narrator appends the [[GAME:{...}]] mechanics block as plain text at the
        // very end of the passage. Stream everything EXCEPT a trailing run that could be
        // that block, so the raw JSON never flashes in the bubble (the `done` event
        // reconciles with the cleaned content anyway). A "[[" that turns out not to be
        // the marker is released as normal prose.
        const flushSafe = () => {
          let safeLen = storyText.length;
          if (rpgEnabled) {
            const cut = storyText.indexOf("[[");
            if (cut !== -1 && /^\[\[G?A?M?E?(:[\s\S]*)?$/.test(storyText.slice(cut))) {
              safeLen = cut;
            }
          }
          if (safeLen > flushedLen) {
            controller.enqueue(sse("delta", { text: storyText.slice(flushedLen, safeLen) }));
            flushedLen = safeLen;
          }
        };

        try {
          for await (const ev of stream) {
            if (ev.type === "text") {
              storyText += ev.text;
              flushSafe();
            } else if (ev.type === "tool") {
              if (ev.name === "generate_image") {
                sawImageTool = true;
              }
              if (ev.argsFragment) {
                toolArgsByIndex.set(
                  ev.index,
                  (toolArgsByIndex.get(ev.index) || "") + ev.argsFragment,
                );
              }
            } else if (ev.type === "done") {
              break;
            }
          }
        } catch (streamReadError) {
          controller.enqueue(
            sse("error", {
              error:
                streamReadError instanceof Error
                  ? streamReadError.message
                  : "Поток истории прервался.",
            }),
          );
          controller.close();
          return;
        }

        // Reassemble the generate_image tool call from the streamed argument
        // fragments and reuse the same parser the buffered path uses.
        const reconstructedToolCalls = sawImageTool
          ? Array.from(toolArgsByIndex.entries()).map(([, args]) => ({
              function: { name: "generate_image", arguments: args },
            }))
          : [];
        const imageToolArgs = parseGenerateImageToolCall(reconstructedToolCalls);

        const trimmedStory = extractStoryText(storyText);
        const rpg = resolveRpgTurn(chatId, rpgEnabled, rpgActors, rpgEnemies, trimmedStory);
        const characterIds = withHeroReference(
          imageToolArgs?.characterIds
            ?.filter((id) => knownCharacterIds.has(id))
            .slice(0, MAX_IMAGE_REFERENCES) || [],
        );
        const assistantMessage: StoryMessage = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: rpg.clean || "Момент повисает, ожидая твоего следующего действия.",
          createdAt: new Date().toISOString(),
          imageRequest:
            includeImageTool && imageToolArgs?.prompt
              ? {
                  needed: true,
                  prompt: finalizeScenePrompt(
                    imageToolArgs.prompt,
                    body.settings.imageStylePrefix,
                  ),
                  mode: body.settings.imageMode,
                  backend: body.settings.imageBackend,
                  aspect: body.settings.aspect,
                  reason: imageToolArgs.reason,
                  characterIds,
                }
              : { needed: false },
        };

        if (chatId) {
          addMessage(chatId, assistantMessage);
        }

        controller.enqueue(
          sse("done", {
            id: assistantMessage.id,
            content: assistantMessage.content,
            imageRequest: assistantMessage.imageRequest,
            events: rpg.events,
          }),
        );
        controller.close();
      },
    });

    return new Response(responseStream, {
      headers: {
        "Content-Type": "text/event-stream; charset=utf-8",
        "Cache-Control": "no-cache, no-transform",
        Connection: "keep-alive",
        "X-Accel-Buffering": "no",
      },
    });
  }

  const { message, error } = await requestStoryMessage(
    body.settings,
    messages,
    includeImageTool,
  );

  if (error) {
    return error;
  }

  const storyText = extractStoryText(message?.content);
  const rpg = resolveRpgTurn(body.chatId, rpgEnabled, rpgActors, rpgEnemies, storyText);
  const imageToolArgs = parseGenerateImageToolCall(message?.tool_calls);

  if (!storyText && !imageToolArgs) {
    return Response.json(
      {
        error: `${provider === "local" ? "Локальная модель" : "Сервер"}: история не получена.`,
        detail: message,
      },
      { status: 502 },
    );
  }

  const characterIds = withHeroReference(
    imageToolArgs?.characterIds
      ?.filter((id) => knownCharacterIds.has(id))
      .slice(0, MAX_IMAGE_REFERENCES) || [],
  );
  const assistantMessage: StoryMessage = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: rpg.clean || "Момент повисает, ожидая твоего следующего действия.",
    createdAt: new Date().toISOString(),
    imageRequest:
      body.settings.imageGenerationEnabled && body.settings.autoImages && imageToolArgs?.prompt
        ? {
            needed: true,
            prompt: finalizeScenePrompt(imageToolArgs.prompt, body.settings.imageStylePrefix),
            mode: body.settings.imageMode,
            backend: body.settings.imageBackend,
            aspect: body.settings.aspect,
            reason: imageToolArgs.reason,
            characterIds,
          }
        : { needed: false },
  };

  if (body.chatId) {
    addMessage(body.chatId, assistantMessage);
  }

  return Response.json({
    id: assistantMessage.id,
    content: assistantMessage.content,
    imageRequest: assistantMessage.imageRequest,
    events: rpg.events,
  });
}
