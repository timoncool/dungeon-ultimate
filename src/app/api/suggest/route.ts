import { z } from "zod";
import { serverEnv } from "@/lib/server-env";

export const runtime = "nodejs";

// "Surprise me" 🎲 field generator. Asks the same local Gemma server that runs
// the story to invent a value for one blank setup field, so the player never
// faces an empty page. Non-streaming, short, no image tool.

const FIELD_PROMPTS: Record<string, string> = {
  world:
    "Придумай ОДНУ свежую, конкретную завязку мира/сценария для приватной текстовой ролевой игры. 1–2 предложения, по-русски, без банальщины (избегай шаблонных таверн и «избранных»). Выведи ТОЛЬКО текст завязки, без преамбул и кавычек.",
  style:
    "Придумай тон и стиль прозы для текстовой ролевой игры — одна короткая ёмкая фраза по-русски (например: «мрачный нуар, скупые рубленые фразы»). Выведи ТОЛЬКО фразу, без преамбул.",
  character:
    "Придумай концепт яркого персонажа для ролевой игры: имя и краткое описание (внешность, характер, одна зацепка). 1–2 предложения по-русски. Выведи ТОЛЬКО текст, без преамбул.",
  opening:
    "Придумай цепляющую первую сцену для старта текстовой ролевой игры: 2–3 предложения живой прозы по-русски, во втором лице («ты…»), заканчивается моментом, приглашающим действие игрока. Выведи ТОЛЬКО сцену.",
};

const requestSchema = z.object({
  field: z.enum(["world", "style", "character", "opening"]),
  context: z.string().max(2000).optional(),
  settings: z
    .object({
      textProvider: z.enum(["local", "custom"]).default("custom"),
      customBaseUrl: z.string().trim().max(500).default(""),
      customModel: z.string().trim().max(200).default(""),
      customApiKey: z.string().trim().max(400).default(""),
    })
    .default({ textProvider: "custom", customBaseUrl: "", customModel: "", customApiKey: "" }),
});

function customChatEndpoint(baseUrl: string): string {
  const url = baseUrl.trim().replace(/\/+$/, "");
  if (/\/chat\/completions$/.test(url)) return url;
  if (/\/v\d+$/.test(url)) return `${url}/chat/completions`;
  return `${url}/v1/chat/completions`;
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const instruction = FIELD_PROMPTS[body.field];

  const baseUrl =
    body.settings.customBaseUrl.trim() ||
    serverEnv("OPENAI_COMPAT_BASE_URL", "http://127.0.0.1:8080/v1");
  const model =
    body.settings.customModel.trim() ||
    serverEnv("OPENAI_COMPAT_MODEL", "gemma-4-12b-uncensored");
  const apiKey = body.settings.customApiKey.trim() || serverEnv("OPENAI_COMPAT_API_KEY");

  const messages = [
    {
      role: "system" as const,
      content:
        "Ты — генератор идей для приватной ролевой игры. Отвечай кратко, по-русски, только запрошенным текстом, без вступлений, пояснений и кавычек.",
    },
    {
      role: "user" as const,
      content: body.context
        ? `${instruction}\n\nКонтекст уже заданного (учитывай, не повторяй дословно):\n${body.context}`
        : instruction,
    },
  ];

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 60_000);
  try {
    const upstream = await fetch(customChatEndpoint(baseUrl), {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
      body: JSON.stringify({
        model,
        messages,
        temperature: 1.0,
        max_tokens: 300,
      }),
      signal: controller.signal,
    });

    if (!upstream.ok) {
      const detail = await upstream.text();
      return Response.json(
        { error: `Сервер не ответил (${upstream.status}).`, detail: detail.slice(0, 500) },
        { status: 502 },
      );
    }

    const data = (await upstream.json()) as {
      choices?: Array<{ message?: { content?: unknown } }>;
    };
    const raw = data?.choices?.[0]?.message?.content;
    const value = (typeof raw === "string" ? raw : "")
      .replace(/^["'«»\s]+|["'«»\s]+$/g, "")
      .trim();

    if (!value) {
      return Response.json({ error: "Пустой ответ от модели." }, { status: 502 });
    }

    return Response.json({ value });
  } catch (error) {
    return Response.json(
      {
        error: "Не удалось получить идею. Запущен ли текстовый сервер?",
        detail: error instanceof Error ? error.message : String(error),
      },
      { status: 502 },
    );
  } finally {
    clearTimeout(timeout);
  }
}
