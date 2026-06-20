import {
  getChat,
  listItems,
  setItemImage,
} from "@/lib/db";
import { serverEnv } from "@/lib/server-env";
import { applyImageStylePrefix, dimensionsForImage } from "@/lib/story-prompt";
import type { GeneratedImage } from "@/lib/types";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

type ItemImageRouteContext = {
  params: Promise<{ chatId: string; itemId: string }>;
};

const RARITY_EN: Record<string, string> = {
  common: "common",
  uncommon: "uncommon",
  rare: "rare",
  epic: "epic",
  legendary: "legendary",
};

const SLOT_EN: Record<string, string> = {
  weapon: "weapon",
  armor: "suit of armor",
  shield: "shield",
  trinket: "magic trinket",
  consumable: "potion or consumable",
  misc: "object",
};

// A standalone item portrait, framed as a clean game inventory icon: one object,
// centred, on a plain dark backdrop — reusable as the image2image reference when
// the item later appears in a scene. Tuned against FLUX.2 [klein]: the item name
// is NOT quoted into the prompt (a quoted name — especially Cyrillic — makes the
// model engrave garbled text on the item), the visual description leads, and a
// strong anti-text clause keeps the icon clean. The description renders well in
// any language, so no English translation is needed.
function itemPortraitPrompt(
  name: string,
  description?: string,
  rarity?: string,
  slot?: string,
): string {
  const category = SLOT_EN[slot ?? "misc"] ?? "object";
  const subject = description?.trim() || name;
  const quality = rarity && RARITY_EN[rarity] ? `${RARITY_EN[rarity]} quality. ` : "";
  return (
    `Fantasy game inventory icon of a ${category}. ${subject}. ${quality}` +
    `A single hero object centered with the whole item in frame, studio product shot ` +
    `on a plain dark background, soft rim light, crisp clean game-asset render. ` +
    `No text, no letters, no words, no runes, no inscriptions, no people.`
  );
}

export async function POST(request: Request, context: ItemImageRouteContext) {
  const { chatId, itemId } = await context.params;

  const chat = getChat(chatId);
  if (!chat) {
    return Response.json({ error: "Chat not found." }, { status: 404 });
  }
  const item = listItems(chatId).find((entry) => entry.id === itemId);
  if (!item) {
    return Response.json({ error: "Item not found." }, { status: 404 });
  }

  const settings = chat.settings;
  if (!settings.imageGenerationEnabled) {
    return Response.json({ error: "Image generation is disabled." }, { status: 409 });
  }

  // An optional body { prompt } overrides the auto-described item prompt.
  let override: string | undefined;
  try {
    const raw = (await request.json()) as { prompt?: string } | null;
    override = raw?.prompt?.trim() || undefined;
  } catch {
    override = undefined;
  }

  // Prefer the narrator's English visual prompt (best FLUX results); fall back to
  // the in-world description, then the bare name.
  const basePrompt =
    override ??
    itemPortraitPrompt(item.name, item.imagePromptEn || item.description, item.rarity, item.slot);
  const prompt = applyImageStylePrefix(basePrompt, settings.imageStylePrefix ?? "");

  const workerUrl = serverEnv("FLUX_WORKER_URL", "http://127.0.0.1:7869");
  const dimensions = dimensionsForImage(settings.imageMode, "square");
  const defaultBackend = serverEnv("IMAGE_SERVER_DEFAULT_BACKEND", "flux-uncensored");
  const requested = settings.imageBackend;
  const backend =
    requested === "mflux-hs" || requested === "sdnq-hs" ? defaultBackend : requested;

  try {
    const upstream = await fetch(`${workerUrl.replace(/\/$/, "")}/generate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        prompt,
        mode: settings.imageMode,
        backend,
        aspect: "square",
        width: dimensions.width,
        height: dimensions.height,
        steps: 4,
        guidance: 0.0,
        references: [],
      }),
    });

    if (!upstream.ok) {
      const detail = await upstream.text();
      return Response.json(
        { error: `Flux worker failed (${upstream.status}).`, detail: detail.slice(0, 1000) },
        { status: 502 },
      );
    }

    const generatedImage = (await upstream.json()) as GeneratedImage;
    const updated = generatedImage?.url
      ? setItemImage(chatId, { id: itemId }, generatedImage.url, { overwrite: true })
      : null;
    return Response.json({ item: updated ?? item, image: generatedImage });
  } catch (error) {
    return Response.json(
      {
        error: "Flux worker is not running.",
        detail: error instanceof Error ? error.message : String(error),
        expected:
          "Откройте Images и нажмите Start, или запустите npm run image:server из папки Open Dungeon.",
      },
      { status: 503 },
    );
  }
}
