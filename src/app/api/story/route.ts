import { z } from "zod";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  addMessage,
  getStorySummary,
  listCharacters,
  setStorySummary,
  updateChatTitleFromInput,
} from "@/lib/db";
import { serverEnv } from "@/lib/server-env";
import { buildStoryMessages, extractStoryText, packStoryHistory } from "@/lib/story-prompt";
import {
  DEFAULT_LOCAL_TEXT_MODEL,
  LOCAL_TEXT_MODEL_IDS,
  localModelContextWindow,
} from "@/lib/text-models";
import type { Attachment, StoryCharacter, StoryMessage } from "@/lib/types";

export const runtime = "nodejs";

const MAX_IMAGE_REFERENCES = 2;
const DEFAULT_MAX_OUTPUT_TOKENS = 16_384;
const MAX_CONFIGURABLE_OUTPUT_TOKENS = 65_536;
const DEFAULT_LOCAL_MAX_OUTPUT_TOKENS = 4_096;
// Rough chars-per-token for English prose, used to budget story history.
const HISTORY_CHARS_PER_TOKEN = 3.6;
// Tokens held back for the system prompt, character portraits, and the reply.
const HISTORY_RESERVE_TOKENS = 8_192;
// Stay comfortably under the context window so a turn can never max it out.
const CONTEXT_SAFETY_MARGIN = 0.9;
const MIN_HISTORY_CHAR_BUDGET = 48_000;
const OPENROUTER_HISTORY_CHAR_BUDGET = 172_800;
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
  mode: z.enum(["turn", "kickoff", "continue", "retry"]).default("turn"),
  input: z.string().min(1),
  messages: z.array(messageSchema).default([]),
  attachments: z.array(attachmentSchema).default([]),
  settings: z.object({
    world: z.string().default(""),
    style: z.string().default(""),
    textProvider: z.enum(["local", "openrouter"]).default("local"),
    localTextModel: z.enum(LOCAL_TEXT_MODEL_IDS).default(DEFAULT_LOCAL_TEXT_MODEL),
    imageMode: z.enum(["fast", "slow"]).default("slow"),
    imageBackend: z.enum(["mflux-hs", "sdnq-hs"]).default("mflux-hs"),
    aspect: z.enum(["square", "portrait", "landscape"]).default("square"),
    autoImages: z.boolean().default(true),
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
            "Detailed visual prompt. Include subject, environment, composition, lighting, camera style, mood, and avoid text overlays. For established characters, describe visible physical features and whether each person is a man or woman; do not rely on character names inside the prompt.",
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
        "Saved character portrait references for visual continuity. Each portrait is labeled with the character's name and exact ID. Use these images to understand what the characters look like, and use exact IDs when calling generate_image.characterIds.",
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
        `Character portrait ${attachedCount}: ${character.name}`,
        `ID: ${character.id}`,
        character.details ? `Details: ${character.details}` : "",
        `The next image is ${character.name}.`,
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
  const parsed = Number.parseInt(serverEnv("LOCAL_TEXT_CONTEXT"), 10);

  if (!Number.isFinite(parsed)) {
    return native;
  }

  return Math.max(2_048, Math.min(parsed, native));
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

async function requestOpenRouterMessage(
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
): Promise<UpstreamResult> {
  const apiKey = serverEnv("OPENROUTER_API_KEY");

  if (!apiKey) {
    return {
      error: Response.json(
        {
          error:
            "Missing OPENROUTER_API_KEY in .env.server or .env.local. Add a key, or switch this chat to the local model in Text Model settings.",
        },
        { status: 500 },
      ),
    };
  }

  const model = serverEnv("OPENROUTER_MODEL", "google/gemini-3.5-flash");
  const baseUrl = serverEnv("OPENROUTER_BASE_URL", "https://openrouter.ai/api/v1");
  const requestPayload: Record<string, unknown> = {
    model,
    messages,
    temperature: 0.9,
    max_tokens: configuredMaxOutputTokens(),
  };

  if (includeImageTool) {
    requestPayload.tools = [generateImageTool];
    requestPayload.tool_choice = "auto";
    requestPayload.parallel_tool_calls = false;
  }

  const upstream = await fetch(`${baseUrl}/chat/completions`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      "HTTP-Referer": serverEnv("OPENROUTER_APP_URL", "http://localhost:3000"),
      "X-OpenRouter-Title": serverEnv("OPENROUTER_APP_TITLE", "Open Dungeon"),
    },
    body: JSON.stringify(requestPayload),
  });

  if (!upstream.ok) {
    const text = await upstream.text();
    return {
      error: Response.json(
        { error: `OpenRouter request failed (${upstream.status}).`, detail: text.slice(0, 1000) },
        { status: upstream.status },
      ),
    };
  }

  const data = (await upstream.json()) as {
    choices?: Array<{ message?: UpstreamChatMessage }>;
  };

  return { message: data?.choices?.[0]?.message };
}

async function requestLocalMessage(
  model: string,
  messages: OpenRouterMessage[],
  includeImageTool: boolean,
  disableThinking = true,
): Promise<UpstreamResult> {
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

  let upstream: Response;
  try {
    upstream = await fetch(`${baseUrl}/api/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(requestPayload),
    });
  } catch {
    return {
      error: Response.json(
        {
          error: `Could not reach Ollama at ${baseUrl}. Start the Ollama app (or \`ollama serve\`), pull the model with \`ollama pull ${model}\`, or switch this chat to OpenRouter in Text Model settings.`,
        },
        { status: 502 },
      ),
    };
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

const SUMMARIZER_SYSTEM = `You maintain the canonical "story so far" memory for an ongoing interactive roleplay. Merge the existing summary with the new passages into one updated summary.

Preserve, with priority: active plot threads and their current state; characters (names, roles, relationships, distinctive physical details); promises, debts, secrets, injuries, and items that could matter later; locations and the order of major events; choices the player made that shaped the story.

Write compact prose in past tense, no headings or lists, at most 500 words. Output only the updated summary.`;

type StoryRequestSettings = z.infer<typeof requestSchema>["settings"];

// Codex-style compaction adapted for stories: passages that scroll out of the
// context window are folded into a rolling summary instead of being forgotten.
// Best-effort — a failed summary never blocks the player's turn.
async function summarizeEvictedPassages(
  settings: StoryRequestSettings,
  existingSummary: string,
  passages: Array<{ role: "user" | "assistant"; content: string }>,
): Promise<string | null> {
  const transcript = passages
    .map((message) => `${message.role === "user" ? "Player" : "Narrator"}: ${message.content}`)
    .join("\n\n");
  const messages: OpenRouterMessage[] = [
    { role: "system", content: SUMMARIZER_SYSTEM },
    {
      role: "user",
      content: `Existing summary:\n${existingSummary || "(none yet)"}\n\nNew passages to fold in:\n${transcript}`,
    },
  ];

  const { message, error } =
    settings.textProvider === "local"
      ? await requestLocalMessage(settings.localTextModel, messages, false)
      : await requestOpenRouterMessage(messages, false);

  if (error) {
    return null;
  }

  const summary = extractStoryText(message?.content);
  return summary ? summary.slice(0, 8_000) : null;
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const characters = body.chatId ? listCharacters(body.chatId) : [];
  const knownCharacterIds = new Set(characters.map((character) => character.id));
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
      : OPENROUTER_HISTORY_CHAR_BUDGET;

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
              content: "The player included visual references for this turn.",
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
  ) as OpenRouterMessage[];
  const characterVisionMessage = buildCharacterVisionMessage(characters);
  const messages = characterVisionMessage
    ? [storyMessages[0], characterVisionMessage, ...storyMessages.slice(1)]
    : storyMessages;
  const { message, error } =
    provider === "local"
      ? await requestLocalMessage(body.settings.localTextModel, messages, body.settings.autoImages)
      : await requestOpenRouterMessage(messages, body.settings.autoImages);

  if (error) {
    return error;
  }

  const storyText = extractStoryText(message?.content);
  const imageToolArgs = parseGenerateImageToolCall(message?.tool_calls);

  if (!storyText && !imageToolArgs) {
    return Response.json(
      {
        error: `${provider === "local" ? "The local model" : "OpenRouter"} returned no story content.`,
        detail: message,
      },
      { status: 502 },
    );
  }

  const characterIds =
    imageToolArgs?.characterIds
      ?.filter((id) => knownCharacterIds.has(id))
      .slice(0, MAX_IMAGE_REFERENCES) || [];
  const assistantMessage: StoryMessage = {
    id: crypto.randomUUID(),
    role: "assistant",
    content: storyText || "The moment hangs there, waiting for what you do next.",
    createdAt: new Date().toISOString(),
    imageRequest:
      body.settings.autoImages && imageToolArgs?.prompt
        ? {
            needed: true,
            prompt: imageToolArgs.prompt,
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
  });
}
