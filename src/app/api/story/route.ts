import { z } from "zod";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  addEvents,
  addItems,
  addMessage,
  getActiveScene,
  getCharacterRpg,
  getCharacterRpgMap,
  getCombatants,
  getHeroCharacter,
  getStorySummary,
  listCharacters,
  listItems,
  saveCharacterRpg,
  setCombatants,
  setStorySummary,
  updateChatTitleFromInput,
} from "@/lib/db";
import { applyGameUpdate, type ActorMap } from "@/lib/rpg/apply";
import { extractGameUpdate, gameUpdateSchema } from "@/lib/rpg/parse";
import { buildRpgSection } from "@/lib/rpg/prompt";
import type { Enemy, GameEvent, GameUpdate, RpgSnapshot } from "@/lib/rpg/types";
import { customChatEndpoint, requestStructuredJson } from "@/lib/llm";
import { serverEnv } from "@/lib/server-env";
import {
  buildStoryMessages,
  extractStoryText,
  finalizeScenePrompt,
  packStoryHistory,
  stripImageArtifacts,
} from "@/lib/story-prompt";
import {
  DEFAULT_LOCAL_TEXT_MODEL,
  LOCAL_TEXT_MODEL_IDS,
  localModelContextWindow,
} from "@/lib/text-models";
import {
  LANGUAGE_PROMPT_NAMES,
  LANGUAGE_VALUES,
  PROSE_SIZE_VALUES,
  RESPONSE_LENGTH_VALUES,
} from "@/lib/types";
import type { Attachment, ImageShot, StoryCharacter, StoryMessage } from "@/lib/types";

export const runtime = "nodejs";

// Total references sent to the worker per image: character portrait(s) + the
// scene-continuity image + a recurring item portrait. FLUX.2 Klein's strongest
// regime is 2-3 references (character first, then scene, then item), so the cap
// is 3 rather than the model's 10-reference ceiling.
const MAX_IMAGE_REFERENCES = 3;
// Of those, how many are character portraits the narrator may name — kept below
// the total so the scene/item references always have room.
const MAX_CHARACTER_REFERENCES = 2;
// Run the structured rules-engine pass: a SEPARATE grammar-constrained call that
// turns the just-written narration into the mechanics JSON. ON by default — this is
// the primary mechanics path now. The schema uses SEMANTIC actor selectors ("hero"
// / an enemy's name), never raw ids, so the local 12B can't poison it with invented
// UUIDs (the old failure mode); ids are resolved server-side in resolveEngineUpdate.
// The narrator writes pure prose and no longer hand-writes a [[GAME]] block.
// Set STRUCTURED_GAME_EVENTS=0 to fall back to narrator [[GAME]] blocks only.
const STRUCTURED_GAME_EVENTS = serverEnv("STRUCTURED_GAME_EVENTS", "1") !== "0";
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
    imageBackend: z.enum(["mflux-hs", "sdnq-hs"]).default("mflux-hs"),
    aspect: z.enum(["square", "portrait", "landscape"]).default("square"),
    imageGenerationEnabled: z.boolean().default(true),
    autoImages: z.boolean().default(true),
    rpgEnabled: z.boolean().default(false),
    randomEvents: z.boolean().default(true),
    diceEnabled: z.boolean().default(true),
    diceSound: z.boolean().default(true),
    diceVolume: z.number().default(55),
    proseSize: z.enum(PROSE_SIZE_VALUES).default("medium"),
    responseLength: z.enum(RESPONSE_LENGTH_VALUES).default("medium"),
    language: z.enum(LANGUAGE_VALUES).default("ru"),
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
      "Request one local FLUX image illustrating the current roleplay moment. Call it every meaningful turn to give the player one key image.",
    parameters: {
      type: "object",
      additionalProperties: false,
      properties: {
        prompt: {
          type: "string",
          description:
            "Detailed visual prompt. Include subject, environment, composition, lighting, camera style, mood, and avoid text overlays. For established characters, describe visible physical features and whether each person is a man or woman; do not rely on character names inside the prompt. Write this prompt in English.",
        },
        location: {
          type: "string",
          description:
            "Short, STABLE label for the physical place of the shot (e.g. 'green meadow', 'crypt of ash'). Reuse the exact same label on every turn the scene stays in that place.",
        },
        sameLocation: {
          type: "boolean",
          description:
            "True if this shot is the SAME physical place as the previous illustrated turn (so the established look is kept and only what changed is changed). False on a new place, a hard cut, or a jump to a close-up.",
        },
        shot: {
          type: "string",
          enum: ["wide", "medium", "close"],
          description: "Camera distance for the shot.",
        },
        reason: {
          type: "string",
          description: "Short private reason this scene benefits from an image.",
        },
        characterIds: {
          type: "array",
          maxItems: MAX_CHARACTER_REFERENCES,
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
  location: z.string().max(120).optional(),
  sameLocation: z.boolean().optional(),
  shot: z.enum(["wide", "medium", "close"]).optional(),
  reason: z.string().optional(),
  characterIds: z.array(z.string()).max(MAX_CHARACTER_REFERENCES).optional(),
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
    {
      role: "system",
      content: `${SUMMARIZER_SYSTEM}\n\nWrite the summary in ${LANGUAGE_PROMPT_NAMES[settings.language]}.`,
    },
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

// Second-pass "rules engine": a SEPARATE grammar-constrained call that reads the
// narration just written + the game state and emits the mechanics as strict JSON.
// Decoupled from the prose (best practice: deterministic rules in code, the model
// only declares WHAT happens), so combat no longer depends on the narrator hand-
// writing a valid [[GAME]] block. The schema constrains the SHAPE; this prompt
// describes the FIELDS (the schema is never shown to the model).
const GAME_ENGINE_SYSTEM = `You are the deterministic D&D rules engine for ONE turn of a solo roleplay (one player hero, plus any enemies). You receive the current game state and the narration of what just happened. Output ONLY a JSON object describing the mechanics this turn triggers. The engine rolls every die and applies all damage/death — you only DECLARE what happens.

⛔ GROUNDING — THE MOST IMPORTANT RULE. Declare ONLY mechanics that are EXPLICITLY present in the narration you are given. Your DEFAULT output is {} (empty). Do NOT invent anything. Specifically:
- Do NOT spawn an enemy unless the narration introduces a NEW foe by name. Use that foe's EXACT name from the prose. Never a foe that isn't in the text.
- Do NOT add damage, an attack, or an effect that the prose does not literally describe happening. Atmosphere (sparks, shadows, tension, a crackling fire) is NOT damage.
- Do NOT copy the placeholder names/numbers from the examples below — they are only format illustrations, never real values.
- If the narration is purely descriptive or social with no dice-worthy risk, output exactly {}.

Refer to combatants by ROLE, never by id: the player character is always "hero"; an enemy is its exact name as written in the state / prose.

Fields (all optional — include ONLY what the prose ACTUALLY shows):
- rolls: an ability check the HERO attempts — [{"ability":"str|dex|con|int|wis|cha","dc":<5-20>,"label":"<short>"}]. Emit a roll WHENEVER the hero attempts something whose outcome is uncertain: sneaking, striking, dodging, picking a lock, persuading/intimidating, jumping, climbing, searching for traps, a saving throw. dc 5 trivial, 10 moderate, 15 hard, 20 very hard. Do NOT roll for safe trivial actions (walking, talking calmly, looking around).
- attacks: a strike the prose describes — [{"attacker":"hero"|"<foe name>","target":"hero"|"<foe name>","ability":"<one ability>","damage":"<dice like 1dN+M>","label":"<short>"}]. Only during a fight the narration is actively depicting.
- spawnEnemies: ONLY a foe the narration newly introduces — [{"name":"<the foe's exact name from the prose>","hp":<small int>,"ac":<10-16>,"level":<1-5>}].
- hpDelta: damage(-)/heal(+) the prose states — [{"who":"hero"|"<foe name>","amount":<signed int>,"reason":"<short>"}].
- applyEffects: a buff/debuff/poison/blessing the prose causes — [{"who":"hero"|"<foe name>","name":"<short>","kind":"buff|debuff","modifiers":{"<ability>":<int>},"turns":<int>}].
- clearEffects: an effect the prose removes — [{"who":"hero"|"<foe name>","name":"<name or *>"}].
- grantItems: loot the HERO actually gains in the prose — [{"name":"<short>","slot":"weapon|armor|shield|trinket|consumable|misc","rarity":"common|uncommon|rare|epic|legendary","damage":"<dice>","withImage":true,"imagePromptEn":"<short ENGLISH visual>"}]. Not for items the hero already owns.

NEVER decide hit/miss/damage/death yourself — the engine does. Output strictly the JSON object, nothing else.`;

const ABILITY_ENUM = ["str", "dex", "con", "int", "wis", "cha"];
const NUM_RECORD = { type: "object", additionalProperties: { type: "number" } };
// JSON-Schema for the engine pass: the local server turns it into a GBNF grammar
// so the call always returns a valid SHAPE. Actors are named semantically ("hero"
// or an enemy's name) — never raw ids — so the 12B can't invent UUIDs; ids are
// resolved server-side (resolveEngineUpdate) into the real GameUpdate apply.ts wants.
const ENGINE_RAW_SCHEMA = {
  type: "object",
  additionalProperties: false,
  properties: {
    rolls: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          ability: { type: "string", enum: ABILITY_ENUM },
          dc: { type: "number" },
          label: { type: "string" },
        },
        required: ["ability", "dc"],
      },
    },
    attacks: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          attacker: { type: "string" },
          target: { type: "string" },
          ability: { type: "string", enum: ABILITY_ENUM },
          damage: { type: "string" },
          label: { type: "string" },
        },
        required: ["target"],
      },
    },
    spawnEnemies: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          name: { type: "string" },
          hp: { type: "number" },
          ac: { type: "number" },
          level: { type: "number" },
          stats: NUM_RECORD,
        },
        required: ["name"],
      },
    },
    hpDelta: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          who: { type: "string" },
          amount: { type: "number" },
          reason: { type: "string" },
        },
        required: ["who", "amount"],
      },
    },
    applyEffects: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          who: { type: "string" },
          name: { type: "string" },
          kind: { type: "string", enum: ["buff", "debuff"] },
          modifiers: NUM_RECORD,
          turns: { type: "number" },
          note: { type: "string" },
        },
        required: ["name"],
      },
    },
    clearEffects: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: { who: { type: "string" }, name: { type: "string" } },
        required: ["name"],
      },
    },
    grantItems: {
      type: "array",
      items: {
        type: "object",
        additionalProperties: false,
        properties: {
          name: { type: "string" },
          slot: {
            type: "string",
            enum: ["weapon", "armor", "shield", "trinket", "consumable", "misc"],
          },
          rarity: {
            type: "string",
            enum: ["common", "uncommon", "rare", "epic", "legendary"],
          },
          description: { type: "string" },
          damage: { type: "string" },
          modifiers: NUM_RECORD,
          qty: { type: "number" },
          withImage: { type: "boolean" },
          imagePromptEn: { type: "string" },
        },
        required: ["name"],
      },
    },
    note: { type: "string" },
  },
};

// Compact, NAME-first state line for the engine pass: the model refers to actors
// semantically ("hero" or an enemy's name), so no ids are exposed to it at all.
function rpgStateForEngine(actors: ActorMap, enemies: Enemy[]): string {
  const lines: string[] = [];
  let first = true;
  for (const [, actor] of actors) {
    const tag = first ? `hero (${actor.name})` : actor.name;
    lines.push(`- ${tag}: HP ${actor.rpg.hp.current}/${actor.rpg.hp.max}, AC ${actor.rpg.ac}`);
    first = false;
  }
  for (const enemy of enemies) {
    if (enemy.rpg.dead) continue;
    lines.push(`- enemy "${enemy.name}": HP ${enemy.rpg.hp.current}/${enemy.rpg.hp.max}, AC ${enemy.rpg.ac}`);
  }
  return lines.length
    ? `CURRENT GAME STATE — refer to combatants by these names ("hero" or an enemy's name):\n${lines.join("\n")}`
    : `No enemies on the field. The hero acts solo.`;
}

// The raw, semantic shape the engine pass returns (ENGINE_RAW_SCHEMA): actors are
// named "hero" / by enemy name, never id. resolveEngineUpdate maps those to the
// real ids the GameUpdate/apply.ts pipeline expects.
type RawEngineUpdate = {
  rolls?: Array<{ ability: string; dc: number; label?: string }>;
  attacks?: Array<{
    attacker?: string;
    target?: string;
    ability?: string;
    damage?: string;
    label?: string;
  }>;
  spawnEnemies?: unknown[];
  hpDelta?: Array<{ who?: string; amount: number; reason?: string }>;
  applyEffects?: Array<{
    who?: string;
    name: string;
    kind?: string;
    modifiers?: Record<string, number>;
    turns?: number;
    note?: string;
  }>;
  clearEffects?: Array<{ who?: string; name: string }>;
  grantItems?: unknown[];
  note?: string;
};

const HERO_WORDS = new Set([
  "hero",
  "player",
  "protagonist",
  "self",
  "me",
  "герой",
  "игрок",
  "протагонист",
  "персонаж",
  "я",
]);

const ABILITY_SET = new Set(ABILITY_ENUM);
const SLOT_SET = new Set(["weapon", "armor", "shield", "trinket", "consumable", "misc"]);
const RARITY_SET = new Set(["common", "uncommon", "rare", "epic", "legendary"]);

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

// Keep only numeric values from a model-supplied stat record, so a stray string
// modifier ("+2") can't fail the whole-update schema validation.
function numericRecord(rec: Record<string, unknown>): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [k, v] of Object.entries(rec)) {
    if (typeof v === "number" && Number.isFinite(v)) out[k] = v;
  }
  return out;
}

// Map one semantic actor reference ("hero" / a synonym / a character or enemy name)
// to a concrete id. Living enemies win over dead ones. Exact match first; the partial
// fallback guards against 1–2 char false positives, requires a whole-word/substring
// hit, and prefers the LONGEST matching name so "Гоблин" never collapses onto a
// short name. Returns undefined when nothing matches (callers DROP rather than guess).
function resolveActorRef(
  ref: string | undefined,
  heroId: string | undefined,
  heroName: string | undefined,
  enemies: Enemy[],
): string | undefined {
  const r = (ref ?? "").trim().toLowerCase();
  if (!r) return undefined;
  if (HERO_WORDS.has(r)) return heroId;
  if (heroName && r === heroName.toLowerCase()) return heroId;
  const living = enemies.filter((enemy) => !enemy.rpg.dead);
  const pools = [living, enemies];
  for (const pool of pools) {
    const exact = pool.find((enemy) => enemy.name.toLowerCase() === r);
    if (exact) return exact.id;
  }
  if (r.length >= 2) {
    for (const pool of pools) {
      const candidates = pool
        .filter((enemy) => {
          const name = enemy.name.toLowerCase();
          if (name.length < 3) return false; // never partial-match a 1–2 char name
          const wholeWord = new RegExp(`(^|\\s)${escapeRegExp(name)}(\\s|$)`).test(r);
          return wholeWord || (r.length >= 3 && name.includes(r));
        })
        .sort((a, b) => b.name.length - a.name.length);
      if (candidates.length) return candidates[0].id;
    }
  }
  return undefined;
}

// Turn the semantic engine output into a real-id GameUpdate. Rolls are always the
// hero's. For attacks/effects/hp, a named actor that does NOT resolve is DROPPED (we
// never silently redirect an enemy's hit onto the hero), and a hero-vs-hero self-hit
// is dropped. Per-field values are validated (enum/number) so one bad field can't make
// the whole-update schema parse fail and discard every mechanic this turn. Returns a
// plain object validated by gameUpdateSchema in the caller.
function resolveEngineUpdate(
  raw: RawEngineUpdate,
  heroId: string | undefined,
  heroName: string | undefined,
  enemies: Enemy[],
): Record<string, unknown> {
  const ref = (value: string | undefined) => resolveActorRef(value, heroId, heroName, enemies);
  const firstEnemyId = () => (enemies.find((enemy) => !enemy.rpg.dead) ?? enemies[0])?.id;
  // Resolve a non-attack "who": absent → the hero; a name that resolves → that actor;
  // a name that does NOT resolve → null, signalling the caller to drop the entry
  // rather than silently land the damage/effect on the hero.
  const resolveWho = (who: string | undefined): string | null | undefined => {
    const s = (who ?? "").trim();
    if (!s) return heroId;
    return ref(who) ?? null;
  };
  const out: Record<string, unknown> = {};

  // Rolls are always the hero's check — skip them entirely when there is no hero
  // character, so a check never gets mis-rolled against an enemy's stats.
  if (raw.rolls?.length && heroId) {
    const rolls = raw.rolls
      .filter((roll) => ABILITY_SET.has(roll.ability))
      .map((roll) => ({
        ability: roll.ability,
        dc: roll.dc,
        ...(roll.label ? { label: roll.label } : {}),
        actorId: heroId,
        kind: "skill",
      }));
    if (rolls.length) out.rolls = rolls;
  }

  if (raw.attacks?.length) {
    const attacks = raw.attacks
      .map((attack) => {
        const attackerRaw = (attack.attacker ?? "").trim();
        let attackerId: string | undefined;
        if (attackerRaw) {
          attackerId = ref(attack.attacker);
          if (!attackerId) return null; // named attacker we can't resolve → drop, don't guess
        } else {
          attackerId = heroId; // unnamed → the hero swings
        }
        let targetId = ref(attack.target);
        if (!targetId) targetId = attackerId === heroId ? firstEnemyId() : heroId;
        if (!targetId) return null;
        if (attackerId && attackerId === targetId) return null; // no self-hit
        return {
          ...(attackerId ? { attackerId } : {}),
          targetId,
          ...(attack.ability && ABILITY_SET.has(attack.ability) ? { ability: attack.ability } : {}),
          ...(attack.damage ? { damage: attack.damage } : {}),
          ...(attack.label ? { label: attack.label } : {}),
        };
      })
      .filter(Boolean);
    if (attacks.length) out.attacks = attacks;
  }

  if (raw.hpDelta?.length) {
    const hpDelta = raw.hpDelta
      .map((entry) => {
        const characterId = resolveWho(entry.who);
        return characterId && typeof entry.amount === "number"
          ? { characterId, amount: entry.amount, ...(entry.reason ? { reason: entry.reason } : {}) }
          : null;
      })
      .filter(Boolean);
    if (hpDelta.length) out.hpDelta = hpDelta;
  }

  if (raw.applyEffects?.length) {
    const applyEffects = raw.applyEffects
      .map((effect) => {
        const characterId = resolveWho(effect.who);
        if (characterId === null) return null; // named-but-unresolved → drop
        const kind =
          effect.kind === "buff" || effect.kind === "debuff" ? effect.kind : undefined;
        return {
          ...(characterId ? { characterId } : {}),
          name: effect.name,
          ...(kind ? { kind } : {}),
          ...(effect.modifiers ? { modifiers: numericRecord(effect.modifiers) } : {}),
          ...(typeof effect.turns === "number" ? { turns: effect.turns } : {}),
          ...(effect.note ? { note: effect.note } : {}),
        };
      })
      .filter(Boolean);
    if (applyEffects.length) out.applyEffects = applyEffects;
  }

  if (raw.clearEffects?.length) {
    const clearEffects = raw.clearEffects
      .map((effect) => {
        const characterId = resolveWho(effect.who);
        if (characterId === null) return null;
        return { ...(characterId ? { characterId } : {}), name: effect.name };
      })
      .filter(Boolean);
    if (clearEffects.length) out.clearEffects = clearEffects;
  }

  if (raw.grantItems?.length) {
    const grantItems = raw.grantItems
      .map((item) => {
        if (!item || typeof item !== "object") return null;
        const o = item as Record<string, unknown>;
        const str = (k: string) => (typeof o[k] === "string" && o[k] ? (o[k] as string) : undefined);
        const name = str("name");
        if (!name) return null; // name is required by the schema
        return {
          name,
          ...(heroId ? { ownerId: heroId } : {}),
          ...(typeof o.slot === "string" && SLOT_SET.has(o.slot) ? { slot: o.slot } : {}),
          ...(typeof o.rarity === "string" && RARITY_SET.has(o.rarity) ? { rarity: o.rarity } : {}),
          ...(str("description") ? { description: str("description") } : {}),
          ...(str("damage") ? { damage: str("damage") } : {}),
          ...(str("imagePromptEn") ? { imagePromptEn: str("imagePromptEn") } : {}),
          ...(o.modifiers && typeof o.modifiers === "object"
            ? { modifiers: numericRecord(o.modifiers as Record<string, unknown>) }
            : {}),
          ...(typeof o.qty === "number" ? { qty: o.qty } : {}),
          ...(typeof o.withImage === "boolean" ? { withImage: o.withImage } : {}),
        };
      })
      .filter(Boolean);
    if (grantItems.length) out.grantItems = grantItems;
  }

  if (raw.spawnEnemies?.length) out.spawnEnemies = raw.spawnEnemies;
  if (raw.note?.trim()) out.note = raw.note; // ignore a whitespace-only note
  return out;
}

// Run the structured rules-engine pass for this turn. Returns a validated
// GameUpdate, or null on failure / a purely-narrative turn — in which case
// resolveRpgTurn falls back to a [[GAME]] block in the prose (if any).
async function requestGameEvent(
  settings: StoryRequestSettings,
  actors: ActorMap,
  enemies: Enemy[],
  playerInput: string,
  narration: string,
): Promise<GameUpdate | null> {
  const messages = [
    {
      role: "system" as const,
      content: `${GAME_ENGINE_SYSTEM}\n\n${rpgStateForEngine(actors, enemies)}\n\nWrite every human-facing string (label/name/reason/note) in ${LANGUAGE_PROMPT_NAMES[settings.language]}.`,
    },
    {
      role: "user" as const,
      content: `Player action: ${playerInput?.trim() || "(none — narrative beat)"}\n\nWhat just happened (narration):\n${narration.slice(-3000)}\n\nOutput the mechanics JSON for this turn ({} if nothing mechanical).`,
    },
  ];
  const result = await requestStructuredJson<RawEngineUpdate>({
    settings,
    messages,
    schema: ENGINE_RAW_SCHEMA,
    temperature: 0.2,
    maxTokens: 500,
    timeoutMs: 60_000,
  });
  if (!result.ok) {
    return null;
  }
  // Resolve semantic actor names -> real ids, then validate the built GameUpdate
  // against the canonical schema (defensive: the grammar guarantees the raw shape,
  // this guarantees the resolved shape apply.ts consumes).
  const heroEntry = actors.entries().next().value as [string, { name: string }] | undefined;
  const built = resolveEngineUpdate(result.data, heroEntry?.[0], heroEntry?.[1]?.name, enemies);
  const parsed = gameUpdateSchema.safeParse(built);
  if (!parsed.success) {
    return null;
  }
  const update = parsed.data as GameUpdate;
  // A lone `note` is decoration, not mechanics — only a non-empty mechanical array
  // counts, so a note-only result falls through (to the [[GAME]] fallback / nothing)
  // instead of masquerading as a real update.
  const mechKeys = [
    "rolls",
    "attacks",
    "spawnEnemies",
    "hpDelta",
    "applyEffects",
    "clearEffects",
    "grantItems",
  ] as const;
  const hasMechanics = mechKeys.some((key) => {
    const value = update[key];
    return Array.isArray(value) && value.length > 0;
  });
  return hasMechanics ? update : null;
}

// Second-pass "cinematographer": a SEPARATE grammar-constrained call that reads the
// finished narration and decides the ONE key image for the moment. Decoupled from
// the prose (the narrator writes pure story now) so the local 12B can never leak an
// invented "[IMAGE_GEN_PROMPT]" block or a half-formed generate_image tool call into
// the passage — the image prompt is produced here, as strict JSON, in English.
const IMAGE_PASS_SYSTEM = `You are the cinematographer for an illustrated roleplay. You receive the latest narration passage and the cast. Decide the ONE key image that best illustrates THIS moment and output it as a JSON object.

- needed: true for almost every meaningful turn — give the player one vivid key image of what just happened (an action, a new place, a character beat, a reveal). false ONLY when the passage is purely meta or a trivial aside.
- prompt: the image description, in ENGLISH (the image model only understands English), as one flowing cinematic paragraph (not a list). Cover, in rough order: subject + their key visible action/pose/expression; the setting and a few establishing details; lighting (source, direction, color, shadows); mood/atmosphere; composition & camera (framing, angle, depth); and a visual style idiom (e.g. cinematic concept art, gritty photoreal render) — never a living artist. Favor concrete observable nouns. Keep one consistent time of day and light logic.
- For established characters do NOT use their names as visual descriptors: describe each by visible features and whether they are a man or woman (age range, build, hair, face, clothing, pose), consistent with how they were described.
- shot: "wide", "medium", or "close".
- location: a short, STABLE label for the physical place of the shot (e.g. "green meadow", "crypt of ash"). Reuse the exact label whenever the scene stays in that place.
- sameLocation: true if this shot is the SAME physical place as the previously illustrated turn (so the established look is kept and only what changed changes); false on a new place, a hard cut, or a jump to a tight close-up.
- characters: the exact NAMES of saved cast members who actually appear in this shot (0–2), or [].

Output strictly the JSON object, nothing else. Never put text, letters, captions, or watermarks into the image.`;

const IMAGE_PASS_SCHEMA = {
  type: "object",
  additionalProperties: false,
  properties: {
    needed: { type: "boolean" },
    prompt: { type: "string" },
    shot: { type: "string", enum: ["wide", "medium", "close"] },
    location: { type: "string" },
    sameLocation: { type: "boolean" },
    characters: { type: "array", items: { type: "string" } },
  },
  required: ["needed", "prompt"],
};

type ImagePassResult = {
  prompt: string;
  shot?: ImageShot;
  location?: string;
  sameLocation?: boolean;
  characterNames: string[];
};

// Run the structured image pass over the finished narration. Returns the scene
// image request (English prompt + continuity hints + cast names), or null for a
// turn that should not be illustrated / on failure.
async function requestImageRequest(
  settings: StoryRequestSettings,
  characters: StoryCharacter[],
  narration: string,
  activeLocation: string | undefined,
): Promise<ImagePassResult | null> {
  if (!narration.trim()) {
    return null;
  }
  // Include each character's gender (folded into details as "Пол: мужской/женский")
  // so the cinematographer draws the right person — names alone leave it guessing,
  // and it defaults a female hero to a man.
  const castLine = characters.length
    ? `Cast (saved characters — depict each with the stated gender, refer to them by these exact names): ${characters
        .map((character) => {
          const g = /Пол:\s*мужской/i.test(character.details)
            ? " [male]"
            : /Пол:\s*женский/i.test(character.details)
              ? " [female]"
              : "";
          return `${character.name}${g}`;
        })
        .join(", ")}.`
    : "No saved characters yet.";
  const sceneLine = activeLocation
    ? `The previously illustrated scene was at: "${activeLocation}". If this moment is the SAME physical place, reuse that exact label and set sameLocation=true.`
    : "No scene has been illustrated yet, so sameLocation=false.";
  const messages = [
    {
      role: "system" as const,
      content: settings.imagePrompt?.trim()
        ? `${settings.imagePrompt.trim()}\n\n${IMAGE_PASS_SYSTEM}`
        : IMAGE_PASS_SYSTEM,
    },
    {
      role: "user" as const,
      content: `${castLine}\n${sceneLine}\n\nNarration to illustrate:\n${narration.slice(-3000)}\n\nOutput the image JSON for this moment.`,
    },
  ];
  const result = await requestStructuredJson<{
    needed?: boolean;
    prompt?: string;
    shot?: string;
    location?: string;
    sameLocation?: boolean;
    characters?: unknown[];
  }>({
    settings,
    messages,
    schema: IMAGE_PASS_SCHEMA,
    temperature: 0.4,
    maxTokens: 500,
    timeoutMs: 60_000,
  });
  if (!result.ok) {
    return null;
  }
  const data = result.data;
  if (!data?.needed || !data.prompt?.trim()) {
    return null;
  }
  const shot: ImageShot | undefined =
    data.shot === "wide" || data.shot === "medium" || data.shot === "close"
      ? data.shot
      : undefined;
  const characterNames = Array.isArray(data.characters)
    ? data.characters.filter((name): name is string => typeof name === "string")
    : [];
  return {
    prompt: data.prompt.trim(),
    shot,
    location: data.location?.trim() || undefined,
    sameLocation: typeof data.sameLocation === "boolean" ? data.sameLocation : undefined,
    characterNames,
  };
}

// Map the image pass's character NAMES back to saved character ids (case- and
// substring-tolerant), capped — the pass speaks names, the reference system speaks
// ids. The hero is folded in separately by withHeroReference at the call site.
function resolveCharacterNames(names: string[], characters: StoryCharacter[]): string[] {
  const ids: string[] = [];
  for (const raw of names) {
    const name = raw.trim().toLowerCase();
    if (!name) continue;
    // Exact first; the partial fallback skips ≤2-char saved names (a 1-letter name
    // would match almost any word), requires a whole-word hit, and prefers the
    // LONGEST candidate so "Иван" never collapses onto "И".
    const exact = characters.find((character) => character.name.toLowerCase() === name);
    const match =
      exact ??
      characters
        .filter((character) => {
          const cn = character.name.toLowerCase();
          if (cn.length < 3) return false;
          const wholeWord = new RegExp(`(^|\\s)${escapeRegExp(cn)}(\\s|$)`).test(name);
          return wholeWord || (name.length >= 3 && cn.includes(name));
        })
        .sort((a, b) => b.name.length - a.name.length)[0];
    if (match && !ids.includes(match.id)) ids.push(match.id);
  }
  return ids.slice(0, MAX_CHARACTER_REFERENCES);
}

// Resolve this turn's mechanics (when RPG is on): roll dice server-side, apply
// HP/death, persist character state + journal events, and return the prose with
// any [[GAME]] block stripped plus the events to surface to the client. The
// structured `providedUpdate` (from the rules-engine pass) wins; a narrator-written
// [[GAME]] block in the text is the fallback.
function resolveRpgTurn(
  chatId: string | undefined,
  enabled: boolean,
  characterActors: ActorMap,
  enemies: Enemy[],
  storyText: string,
  randomEvents = false,
  providedUpdate?: GameUpdate | null,
): { clean: string; events: GameEvent[]; snapshot?: RpgSnapshot } {
  if (!enabled) {
    return { clean: storyText, events: [] };
  }
  // Always strip a [[GAME]] block from the prose (clean text), but prefer the
  // structured engine update over whatever the narrator may have hand-written.
  const { clean, update: textUpdate } = extractGameUpdate(storyText);
  const update = providedUpdate ?? textUpdate;
  if (!update) {
    return { clean, events: [] };
  }
  // Combined actor map: player characters + current enemies, so the engine can
  // resolve attacks/HP against either side and target newly-spawned foes.
  const actors: ActorMap = new Map(characterActors);
  const enemyIds = new Set(enemies.map((enemy) => enemy.id));
  for (const enemy of enemies) actors.set(enemy.id, { name: enemy.name, rpg: enemy.rpg });

  // The protagonist is the first character actor (getCharacterRpgMap orders by
  // created_at ASC); random events and effect fallbacks target them.
  const heroId = characterActors.keys().next().value as string | undefined;
  // Freeze the pre-turn enemy roster BEFORE applyGameUpdate runs: the actor map
  // holds each enemy.rpg BY REFERENCE (line above), so the engine mutates these
  // very objects in place. Cloning now is the only way the rollback snapshot can
  // capture pre-turn enemy HP/dead/effects instead of the post-turn state.
  const preTurnEnemies = chatId ? enemies.map((enemy) => structuredClone(enemy)) : [];
  const { events, changed, items, spawnedEnemies } = applyGameUpdate(update, actors, {
    heroId,
    randomEvents,
  });
  let snapshot: RpgSnapshot | undefined;
  if (chatId) {
    // Pre-turn snapshot for Retry/Erase rollback. chars come from getCharacterRpg
    // (still the base — saves happen below); combatants is the pre-mutation clone;
    // itemIds/eventIds are the NEW rows this turn created, deleted on rollback.
    snapshot = {
      chars: {},
      combatants: preTurnEnemies,
      itemIds: items.map((item) => item.id),
      eventIds: events.map((event) => event.id),
    };
    for (const id of characterActors.keys()) {
      const base = getCharacterRpg(chatId, id);
      if (base) snapshot.chars[id] = base;
    }
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
          // Persist the ticked/applied effects (they live on the base rpg, folded
          // into the derived stats just like gear).
          base.effects = actor.rpg.effects;
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
  return { clean, events, snapshot };
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const characters = body.chatId ? listCharacters(body.chatId) : [];
  const knownCharacterIds = new Set(characters.map((character) => character.id));
  // The protagonist = the first character created. Resolve via the canonical
  // helper (created_at ASC, rowid ASC) so this matches the RPG path's hero
  // (getCharacterRpgMap) even when two characters share a created_at timestamp.
  const heroId = body.chatId ? getHeroCharacter(body.chatId)?.id : undefined;
  // Ensure the player character rides along as a visual reference whenever a
  // scene is illustrated, so the evolving-hero image2image reference attaches
  // and refreshes even if the narrator didn't name them. Keeps within the
  // reference cap and only adds a known id once.
  const withHeroReference = (ids: string[]): string[] => {
    if (!heroId || ids.includes(heroId) || ids.length >= MAX_CHARACTER_REFERENCES) {
      return ids;
    }
    return [heroId, ...ids].slice(0, MAX_CHARACTER_REFERENCES);
  };
  const rpgEnabled = body.settings.rpgEnabled;
  const rpgActors: ActorMap =
    rpgEnabled && body.chatId ? getCharacterRpgMap(body.chatId) : new Map();
  const rpgEnemies: Enemy[] = rpgEnabled && body.chatId ? getCombatants(body.chatId) : [];
  const rpgSection = rpgEnabled
    ? buildRpgSection(
        rpgActors,
        body.chatId ? listItems(body.chatId) : [],
        rpgEnemies,
        body.settings.language,
      )
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
    body.settings.language,
  ) as OpenRouterMessage[];
  const characterVisionMessage = buildCharacterVisionMessage(characters);
  const messages = characterVisionMessage
    ? [storyMessages[0], characterVisionMessage, ...storyMessages.slice(1)]
    : storyMessages;
  // Images are produced by the SEPARATE structured image pass (requestImageRequest)
  // over the finished narration — the narrator call no longer carries the
  // generate_image tool, so the local 12B can't leak a half-formed image call into
  // the prose. `wantImage` gates the pass; the model never sees an image tool.
  const wantImage = body.settings.imageGenerationEnabled && body.settings.autoImages;
  // Off by default — the structured image pass handles illustration. Opt back into the
  // native generate_image tool (e.g. for a strong tool-calling backend) via env; the
  // tool-call parsing stays wired as the fallback so it's a real switch, not dead code.
  const includeImageTool = serverEnv("NARRATOR_IMAGE_TOOL", "") === "1";

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

        // Hold back any trailing run that could be the START of a model artifact and
        // stream only the safe prefix, so a raw marker never flashes in the bubble
        // (the `done` event reconciles with the fully cleaned content anyway). Covers a
        // [[GAME:{...}]] mechanics block, an invented [IMAGE_GEN_PROMPT]/generate_image
        // image call, a ```json fence, and a lone trailing "[". Anchored to end-of-
        // string so brackets earlier in real prose are released as normal text.
        // Each branch matches only a TRUE trailing run (anchored to $): a partial
        // [[GAME / [IMAGE marker, a `generate_image` (optionally opening a same-line
        // [ or {), a partial ``` fence, or a lone trailing "[". The generate_image and
        // backtick branches are bounded (no run-to-$) so a `generate_image` word or a
        // backtick EARLIER in real prose is released as normal text, not held back.
        const TRAILING_ARTIFACT =
          /(?:\[\[?\s*G(?:A(?:M(?:E(?:\s*:[\s\S]*)?)?)?)?|\[\s*I(?:M(?:A(?:G(?:E[\s\S]*)?)?)?)?|(?:call:)?\s*generate_image(?:\s*[[{][^\n]*)?|`{1,3}[a-z]*|\[)$/i;
        const flushSafe = () => {
          let safeLen = storyText.length;
          const cut = storyText.search(TRAILING_ARTIFACT);
          if (cut !== -1) {
            safeLen = cut;
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

        try {
        const trimmedStory = stripImageArtifacts(extractStoryText(storyText));
        // Two structured passes over the finished narration: the RPG rules engine
        // (events) and the cinematographer (the scene image). Run SEQUENTIALLY — the
        // local model server is single-slot, so firing both at once just makes them
        // collide. The narrator wrote pure prose; these add the mechanics + image.
        const activeLocation = chatId ? getActiveScene(chatId)?.location : undefined;
        const gameUpdate =
          rpgEnabled && STRUCTURED_GAME_EVENTS
            ? await requestGameEvent(body.settings, rpgActors, rpgEnemies, body.input, trimmedStory)
            : null;
        const imagePass = wantImage
          ? await requestImageRequest(body.settings, characters, trimmedStory, activeLocation)
          : null;
        const rpg = resolveRpgTurn(
          chatId,
          rpgEnabled,
          rpgActors,
          rpgEnemies,
          trimmedStory,
          body.settings.randomEvents,
          gameUpdate,
        );
        const assistantMessage: StoryMessage = {
          id: crypto.randomUUID(),
          role: "assistant",
          content: rpg.clean || "Момент повисает, ожидая твоего следующего действия.",
          createdAt: new Date().toISOString(),
          imageRequest: imagePass?.prompt
            ? {
                needed: true,
                prompt: finalizeScenePrompt(imagePass.prompt, body.settings.imageStylePrefix),
                mode: body.settings.imageMode,
                backend: body.settings.imageBackend,
                aspect: body.settings.aspect,
                characterIds: withHeroReference(
                  resolveCharacterNames(imagePass.characterNames, characters),
                ),
                location: imagePass.location,
                sameLocation: imagePass.sameLocation,
                shot: imagePass.shot,
              }
            : includeImageTool && imageToolArgs?.prompt
              ? {
                  needed: true,
                  prompt: finalizeScenePrompt(imageToolArgs.prompt, body.settings.imageStylePrefix),
                  mode: body.settings.imageMode,
                  backend: body.settings.imageBackend,
                  aspect: body.settings.aspect,
                  reason: imageToolArgs.reason,
                  characterIds: withHeroReference(
                    imageToolArgs.characterIds
                      ?.filter((id) => knownCharacterIds.has(id))
                      .slice(0, MAX_CHARACTER_REFERENCES) || [],
                  ),
                  location: imageToolArgs.location,
                  sameLocation: imageToolArgs.sameLocation,
                  shot: imageToolArgs.shot,
                }
              : { needed: false },
          rpgSnapshot: rpg.snapshot,
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
        } catch (resolveError) {
          // Dice/DB/persistence AFTER the stream read must not hang the client:
          // emit an error event and close instead of rejecting start() with no `done`.
          // The assistant message is saved LAST (addMessage), so on most throw paths
          // (resolveRpgTurn / dice / DB) it was never persisted — the client drops the
          // orphan bubble; a reload restores whatever the DB actually committed.
          controller.enqueue(
            sse("error", {
              error:
                resolveError instanceof Error
                  ? resolveError.message
                  : "Не удалось завершить ход.",
            }),
          );
          controller.close();
        }
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

  const storyText = stripImageArtifacts(extractStoryText(message?.content));
  const imageToolArgs = parseGenerateImageToolCall(message?.tool_calls);
  const activeLocation = body.chatId ? getActiveScene(body.chatId)?.location : undefined;
  // Two structured passes over the finished narration, SEQUENTIAL (mirrors the
  // streaming path): the single-slot model server can't serve both at once. The RPG
  // rules engine (events) then the cinematographer (image).
  const gameUpdate =
    rpgEnabled && STRUCTURED_GAME_EVENTS
      ? await requestGameEvent(body.settings, rpgActors, rpgEnemies, body.input, storyText)
      : null;
  const imagePass = wantImage
    ? await requestImageRequest(body.settings, characters, storyText, activeLocation)
    : null;
  const rpg = resolveRpgTurn(
    body.chatId,
    rpgEnabled,
    rpgActors,
    rpgEnemies,
    storyText,
    body.settings.randomEvents,
    gameUpdate,
  );

  if (!storyText && !imagePass && !imageToolArgs) {
    return Response.json(
      {
        error: `${provider === "local" ? "Локальная модель" : "Сервер"}: история не получена.`,
        detail: message,
      },
      { status: 502 },
    );
  }

  const assistantMessage: StoryMessage = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: rpg.clean || "Момент повисает, ожидая твоего следующего действия.",
    createdAt: new Date().toISOString(),
    imageRequest: imagePass?.prompt
      ? {
          needed: true,
          prompt: finalizeScenePrompt(imagePass.prompt, body.settings.imageStylePrefix),
          mode: body.settings.imageMode,
          backend: body.settings.imageBackend,
          aspect: body.settings.aspect,
          characterIds: withHeroReference(
            resolveCharacterNames(imagePass.characterNames, characters),
          ),
          location: imagePass.location,
          sameLocation: imagePass.sameLocation,
          shot: imagePass.shot,
        }
      : includeImageTool && imageToolArgs?.prompt
        ? {
            needed: true,
            prompt: finalizeScenePrompt(imageToolArgs.prompt, body.settings.imageStylePrefix),
            mode: body.settings.imageMode,
            backend: body.settings.imageBackend,
            aspect: body.settings.aspect,
            reason: imageToolArgs.reason,
            characterIds: withHeroReference(
              imageToolArgs.characterIds
                ?.filter((id) => knownCharacterIds.has(id))
                .slice(0, MAX_CHARACTER_REFERENCES) || [],
            ),
            location: imageToolArgs.location,
            sameLocation: imageToolArgs.sameLocation,
            shot: imageToolArgs.shot,
          }
        : { needed: false },
    rpgSnapshot: rpg.snapshot,
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
