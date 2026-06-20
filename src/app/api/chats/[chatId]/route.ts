import { z } from "zod";
import { deleteChat, getCharacterRpgMap, getChat, listItems, updateChat } from "@/lib/db";
import { LOCAL_TEXT_MODEL_IDS } from "@/lib/text-models";
import { PROSE_SIZE_VALUES, RESPONSE_LENGTH_VALUES } from "@/lib/types";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type ChatRouteContext = {
  params: Promise<{ chatId: string }>;
};

const settingsSchema = z.object({
  world: z.string().optional(),
  style: z.string().optional(),
  narratorPrompt: z.string().optional(),
  imagePrompt: z.string().optional(),
  textProvider: z.enum(["local", "custom"]).optional(),
  localTextModel: z.enum(LOCAL_TEXT_MODEL_IDS).optional(),
  customBaseUrl: z.string().optional(),
  customModel: z.string().optional(),
  customApiKey: z.string().optional(),
  imageMode: z.enum(["fast", "slow"]).optional(),
  imageBackend: z.enum(["mflux-hs", "sdnq-hs", "flux-uncensored"]).optional(),
  aspect: z.enum(["square", "portrait", "landscape"]).optional(),
  imageGenerationEnabled: z.boolean().optional(),
  autoImages: z.boolean().optional(),
  rpgEnabled: z.boolean().optional(),
  diceEnabled: z.boolean().optional(),
  diceSound: z.boolean().optional(),
  diceVolume: z.number().optional(),
  proseSize: z.enum(PROSE_SIZE_VALUES).optional(),
  responseLength: z.enum(RESPONSE_LENGTH_VALUES).optional(),
  voice: z.string().optional(),
  autoplay: z.boolean().optional(),
  ttsVolume: z.number().optional(),
  ttsSpeed: z.number().optional(),
});

const updateChatSchema = z.object({
  title: z.string().trim().min(1).optional(),
  settings: settingsSchema.optional(),
});

export async function GET(_request: Request, context: ChatRouteContext) {
  const { chatId } = await context.params;
  const chat = getChat(chatId);

  if (!chat) {
    return Response.json({ error: "Chat not found." }, { status: 404 });
  }

  // In D&D mode, hydrate the player HUD + inventory alongside the chat so they
  // survive a reload (they are otherwise only built from live turn events).
  if (!chat.settings.rpgEnabled) {
    return Response.json({ chat });
  }
  const heroId = chat.characters[0]?.id ?? null;
  const heroRpg = heroId ? getCharacterRpgMap(chatId).get(heroId)?.rpg ?? null : null;
  return Response.json({ chat, heroId, heroRpg, items: listItems(chatId) });
}

export async function PATCH(request: Request, context: ChatRouteContext) {
  const { chatId } = await context.params;
  const body = updateChatSchema.parse(await request.json());
  const chat = updateChat(chatId, body);

  if (!chat) {
    return Response.json({ error: "Chat not found." }, { status: 404 });
  }

  return Response.json({ chat });
}

export async function DELETE(_request: Request, context: ChatRouteContext) {
  const { chatId } = await context.params;
  const deleted = deleteChat(chatId);

  if (!deleted) {
    return Response.json({ error: "Chat not found." }, { status: 404 });
  }

  return Response.json({ ok: true });
}
