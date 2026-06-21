import { z } from "zod";
import { requestStructuredJson } from "@/lib/llm";
import { LANGUAGE_PROMPT_NAMES, LANGUAGE_VALUES } from "@/lib/types";

export const runtime = "nodejs";

// "Fill it for me" in the New Story dialog: one grammar-constrained call invents a
// protagonist (name + persona) and a first-scene hint for the chosen setting, so
// the player can start with one click. Structured JSON, like the action chips.

const requestSchema = z.object({
  setting: z.string().max(4000).default(""),
  settings: z
    .object({
      customBaseUrl: z.string().trim().max(500).default(""),
      customModel: z.string().trim().max(200).default(""),
      customApiKey: z.string().trim().max(400).default(""),
      language: z.enum(LANGUAGE_VALUES).default("ru"),
    })
    .default({ customBaseUrl: "", customModel: "", customApiKey: "", language: "ru" }),
});

const SETUP_SCHEMA = {
  type: "object",
  additionalProperties: false,
  properties: {
    name: { type: "string", minLength: 1, maxLength: 40 },
    persona: { type: "string", minLength: 1, maxLength: 90 },
    hint: { type: "string", minLength: 1, maxLength: 200 },
  },
  required: ["name", "persona", "hint"],
};

export async function POST(request: Request) {
  const parsed = requestSchema.safeParse(await request.json().catch(() => null));
  if (!parsed.success) {
    return Response.json({ ok: false }, { status: 400 });
  }
  const body = parsed.data;
  const lang = LANGUAGE_PROMPT_NAMES[body.settings.language];
  const messages = [
    {
      role: "system" as const,
      content: `Ты придумываешь яркого протагониста для текстовой ролевой игры. Пиши на ${lang}, конкретно и сочно, без банальщины (никаких «избранных» и шаблонных таверн).`,
    },
    {
      role: "user" as const,
      content: `Сеттинг: ${body.setting || "(свободный)"}\n\nПридумай протагониста и зацепку первой сцены. Верни JSON-объект {"name":"<имя, 1–3 слова>","persona":"<кто это: роль/занятие + одна зацепка, 3–8 слов>","hint":"<короткая затравка первой сцены, 1 фраза>"}.`,
    },
  ];
  const result = await requestStructuredJson<{ name?: string; persona?: string; hint?: string }>({
    settings: body.settings,
    messages,
    schema: SETUP_SCHEMA,
    temperature: 0.95,
    maxTokens: 220,
    timeoutMs: 45_000,
  });
  if (!result.ok) {
    return Response.json({ ok: false }, { status: 502 });
  }
  return Response.json({
    ok: true,
    name: (result.data.name ?? "").trim(),
    persona: (result.data.persona ?? "").trim(),
    hint: (result.data.hint ?? "").trim(),
  });
}
