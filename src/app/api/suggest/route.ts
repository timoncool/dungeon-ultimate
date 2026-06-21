import { z } from "zod";
import { requestChatCompletion } from "@/lib/llm";
import { LANGUAGE_VALUES } from "@/lib/types";
import { promptsFor } from "@/lib/prompts";

export const runtime = "nodejs";

// "Surprise me" 🎲 field generator. Asks the same local Gemma server that runs
// the story to invent a value for one blank setup field, so the player never
// faces an empty page. Non-streaming, short, no image tool. Prompts come from
// the localized set, already written in the player's chosen language.

const requestSchema = z.object({
  field: z.enum(["world", "style", "character", "opening"]),
  context: z.string().max(2000).optional(),
  settings: z
    .object({
      textProvider: z.enum(["local", "custom"]).default("custom"),
      customBaseUrl: z.string().trim().max(500).default(""),
      customModel: z.string().trim().max(200).default(""),
      customApiKey: z.string().trim().max(400).default(""),
      language: z.enum(LANGUAGE_VALUES).default("ru"),
    })
    .default({
      textProvider: "custom",
      customBaseUrl: "",
      customModel: "",
      customApiKey: "",
      language: "ru",
    }),
});

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());
  const prompts = promptsFor(body.settings.language).suggest;
  const instruction = prompts.fields[body.field];

  const messages = [
    {
      role: "system" as const,
      content: prompts.system,
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
