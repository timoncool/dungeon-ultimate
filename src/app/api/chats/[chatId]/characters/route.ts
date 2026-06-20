import { z } from "zod";
import { createCharacter, listCharacters } from "@/lib/db";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type CharactersRouteContext = {
  params: Promise<{ chatId: string }>;
};

const attachmentSchema = z.object({
  id: z.string(),
  name: z.string(),
  type: z.string(),
  url: z.string(),
  dataUrl: z.string().optional(),
});

const createCharacterSchema = z.object({
  name: z.string().trim().min(1),
  details: z.string().optional(),
  inventory: z.string().optional(),
  skills: z.string().optional(),
  spells: z.string().optional(),
  portrait: attachmentSchema.optional(),
  // Optional per-character TTS voice id (same id space as POST /api/tts.voice).
  voice: z.string().trim().max(200).optional(),
});

export async function GET(_request: Request, context: CharactersRouteContext) {
  const { chatId } = await context.params;
  return Response.json({ characters: listCharacters(chatId) });
}

export async function POST(request: Request, context: CharactersRouteContext) {
  const { chatId } = await context.params;
  const body = createCharacterSchema.parse(await request.json());
  const character = createCharacter(chatId, body);

  return Response.json({ character }, { status: 201 });
}
