import { z } from "zod";
import { requestChatCompletion } from "@/lib/llm";

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

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const instruction = FIELD_PROMPTS[body.field];

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

  const result = await requestChatCompletion({
    settings: body.settings,
    messages,
    temperature: 1.0,
    maxTokens: 300,
    timeoutMs: 60_000,
  });

  if (!result.ok) {
    // status 0 = the request never reached the server (down / timed out).
    return Response.json(
      result.status
        ? { error: `Сервер не ответил (${result.status}).`, detail: result.detail }
        : { error: "Не удалось получить идею. Запущен ли текстовый сервер?", detail: result.detail },
      { status: 502 },
    );
  }

  const value = result.content.replace(/^["'«»\s]+|["'«»\s]+$/g, "").trim();
  if (!value) {
    return Response.json({ error: "Пустой ответ от модели." }, { status: 502 });
  }
  return Response.json({ value });
}
