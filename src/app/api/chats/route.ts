import { z } from "zod";
import { createChat, listChats } from "@/lib/db";
import { LOCAL_TEXT_MODEL_IDS } from "@/lib/text-models";
import { PROSE_SIZE_VALUES, RESPONSE_LENGTH_VALUES } from "@/lib/types";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

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
  imageStylePrefix: z.string().optional(),
  imageMode: z.enum(["fast", "slow"]).optional(),
  imageBackend: z.enum(["mflux-hs", "sdnq-hs"]).optional(),
  aspect: z.enum(["square", "portrait", "landscape"]).optional(),
  imageGenerationEnabled: z.boolean().optional(),
  autoImages: z.boolean().optional(),
  // RPG + narration flags: must be accepted at create time too, otherwise Zod
  // strips them and a brand-new story silently starts with D&D / companion OFF.
  rpgEnabled: z.boolean().optional(),
  randomEvents: z.boolean().optional(),
  diceEnabled: z.boolean().optional(),
  diceSound: z.boolean().optional(),
  diceVolume: z.number().optional(),
  antiRepetition: z.boolean().optional(),
  causeAwareEnding: z.boolean().optional(),
  multiVoice: z.boolean().optional(),
  companion: z.boolean().optional(),
  proseSize: z.enum(PROSE_SIZE_VALUES).optional(),
  responseLength: z.enum(RESPONSE_LENGTH_VALUES).optional(),
  voice: z.string().optional(),
  autoplay: z.boolean().optional(),
  ttsVolume: z.number().optional(),
  ttsSpeed: z.number().optional(),
});

const createChatSchema = z.object({
  title: z.string().trim().min(1).optional(),
  settings: settingsSchema.optional(),
});

export async function GET() {
  return Response.json({ chats: listChats() });
}

export async function POST(request: Request) {
  const raw = await request.json().catch(() => ({}));
  const body = createChatSchema.parse(raw);
  const chat = createChat(body.settings, body.title);

  return Response.json({ chat }, { status: 201 });
}
