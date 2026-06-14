import { z } from "zod";
import { deleteCharacter, updateCharacter } from "@/lib/db";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type CharacterRouteContext = {
  params: Promise<{ chatId: string; characterId: string }>;
};

const attachmentSchema = z.object({
  id: z.string(),
  name: z.string(),
  type: z.string(),
  url: z.string(),
  dataUrl: z.string().optional(),
});

const updateCharacterSchema = z.object({
  name: z.string().trim().min(1).optional(),
  details: z.string().optional(),
  inventory: z.string().optional(),
  skills: z.string().optional(),
  spells: z.string().optional(),
  portrait: attachmentSchema.nullable().optional(),
});

export async function PATCH(request: Request, context: CharacterRouteContext) {
  const { chatId, characterId } = await context.params;
  const body = updateCharacterSchema.parse(await request.json());
  const character = updateCharacter(chatId, characterId, body);

  if (!character) {
    return Response.json({ error: "Character not found." }, { status: 404 });
  }

  return Response.json({ character });
}

export async function DELETE(_request: Request, context: CharacterRouteContext) {
  const { chatId, characterId } = await context.params;
  const deleted = deleteCharacter(chatId, characterId);

  if (!deleted) {
    return Response.json({ error: "Character not found." }, { status: 404 });
  }

  return Response.json({ ok: true });
}
