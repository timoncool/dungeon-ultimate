import { z } from "zod";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  getCharacterReference,
  getHeroCharacter,
  getMessageContext,
  listItems,
  setCharacterReference,
  setItemImage,
  updateMessageGeneratedImage,
} from "@/lib/db";
import { callFluxWorker, resolveImageBackend } from "@/lib/flux-worker";
import { dimensionsForImage } from "@/lib/story-prompt";
import type { Item } from "@/lib/rpg/types";
import type { Attachment } from "@/lib/types";

export const runtime = "nodejs";

const MAX_IMAGE_REFERENCES = 2;

const referenceSchema = z.object({
  id: z.string(),
  name: z.string(),
  type: z.string(),
  url: z.string(),
  dataUrl: z.string().optional(),
});

const requestSchema = z.object({
  messageId: z.string().optional(),
  prompt: z.string().min(1),
  mode: z.enum(["fast", "slow"]).default("slow"),
  backend: z.enum(["mflux-hs", "sdnq-hs", "flux-uncensored"]).default("flux-uncensored"),
  aspect: z.enum(["square", "portrait", "landscape"]).default("square"),
  seed: z.number().int().optional(),
  references: z.array(referenceSchema).default([]),
});

type ImageReference = z.infer<typeof referenceSchema>;

const mimeByExtension: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
  gif: "image/gif",
};

// Resolve a public-served URL ("/generated/x.png", "/uploads/y.jpg") to bytes on
// disk and inline them as a data URL. The MPS flux_worker only reads dataUrl, and
// inlining also lets the optimized worker skip a re-read; both keep working when
// references are injected server-side rather than coming from the browser.
function inlineLocalReference(reference: ImageReference): ImageReference {
  if (reference.dataUrl?.startsWith("data:image/")) {
    return reference;
  }
  if (!reference.url.startsWith("/")) {
    return reference;
  }

  const extension = reference.url.split("?")[0].split("#")[0].split(".").pop()?.toLowerCase();
  const mime = extension ? mimeByExtension[extension] : undefined;
  if (!mime) {
    return reference;
  }

  const publicDir = path.join(process.cwd(), "public");
  const cleanUrl = reference.url.split("?")[0].split("#")[0];
  const localPath = path.resolve(publicDir, cleanUrl.replace(/^\/+/, ""));
  if (!localPath.startsWith(`${publicDir}${path.sep}`) || !existsSync(localPath)) {
    return reference;
  }

  try {
    const encoded = readFileSync(localPath).toString("base64");
    return { ...reference, dataUrl: `data:${mime};base64,${encoded}` };
  } catch {
    return reference;
  }
}

// An attachment becomes a worker reference; the worker keys off name/url/dataUrl.
function attachmentToReference(attachment: Attachment): ImageReference {
  return {
    id: attachment.id,
    name: attachment.name,
    type: attachment.type,
    url: attachment.url,
    dataUrl: attachment.dataUrl,
  };
}

// Add a reference if there's room and it isn't already present (dedup by URL,
// then by id). Mutates and returns the same array for chaining.
function pushUniqueReference(references: ImageReference[], next: ImageReference): ImageReference[] {
  if (references.length >= MAX_IMAGE_REFERENCES) {
    return references;
  }
  const exists = references.some(
    (reference) =>
      (reference.url && reference.url === next.url) || (reference.id && reference.id === next.id),
  );
  if (!exists) {
    references.push(next);
  }
  return references;
}

// Does the prompt mention this item name? Case-insensitive substring, with an
// ASCII word-boundary guard so short Latin names ("axe") don't match inside
// other words ("axed"). Plain `includes` for non-ASCII names (e.g. Cyrillic),
// where JS \b is unreliable; a >=3 char floor keeps that from over-matching.
function mentionsName(haystackLower: string, name: string): boolean {
  const needle = name.trim().toLowerCase();
  if (needle.length < 3) {
    return false;
  }
  if (/^[\x00-\x7f]+$/.test(needle)) {
    const pattern = new RegExp(`\\b${needle.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`);
    return pattern.test(haystackLower);
  }
  return haystackLower.includes(needle);
}

// One scan of the chat's items: those whose trimmed name the prompt mentions,
// longest name first so they win the limited reference slots. Loaded/matched
// once per request; callers derive portrait references and tagging names below.
function itemsMentionedIn(chatId: string, prompt: string): Item[] {
  const haystack = prompt.toLowerCase();
  return listItems(chatId)
    .filter((item) => item.name.trim() && mentionsName(haystack, item.name))
    .sort((a, b) => b.name.length - a.name.length);
}

// Stored items, with a generated portrait, illustrated consistently as references.
function recurringItemReferences(items: Item[]): Attachment[] {
  return items
    .filter((item) => item.imageUrl)
    .map((item) => ({
      id: `item-${item.id}`,
      name: item.name,
      type: "image/png",
      url: item.imageUrl as string,
    }));
}

export async function POST(request: Request) {
  const body = requestSchema.parse(await request.json());

  // Recover img2img context from the message this image belongs to. Best-effort:
  // a missing/legacy message just means no server-side reference enrichment.
  const context = body.messageId ? getMessageContext(body.messageId) : null;
  const chatId = context?.chatId;
  const sceneCharacterIds = context?.imageRequest?.characterIds ?? [];
  const hero = chatId ? getHeroCharacter(chatId) : null;
  // The hero appears when explicitly referenced, or in an unattributed scene
  // (no characterIds) — the protagonist is the default subject of the story.
  const heroInScene =
    !!hero && (sceneCharacterIds.length === 0 || sceneCharacterIds.includes(hero.id));

  // Start from the client-provided references, then enrich. Inlining first means
  // injected generated images are carried as data URLs for the MPS worker too.
  const references: ImageReference[] = body.references
    .slice(0, MAX_IMAGE_REFERENCES)
    .map(inlineLocalReference);

  // (1) Evolving hero reference: prefer the latest illustrated look over the
  // static portrait the client may have attached, and ensure it's present.
  if (chatId && heroInScene && hero) {
    const heroReference = getCharacterReference(chatId, hero.id);
    if (heroReference) {
      const inlined = inlineLocalReference(attachmentToReference(heroReference));
      // Drop a stale portrait of the same hero so the evolving ref can take the
      // slot, then (re)insert the evolving reference at the front.
      const heroUrls = new Set(
        [hero.portrait?.url, heroReference.url].filter((value): value is string => !!value),
      );
      const withoutHeroPortrait = references.filter(
        (reference) => !heroUrls.has(reference.url) || reference.url === inlined.url,
      );
      references.length = 0;
      references.push(...withoutHeroPortrait);
      if (!references.some((reference) => reference.url === inlined.url)) {
        references.unshift(inlined);
        references.splice(MAX_IMAGE_REFERENCES);
      }
    }
  }

  // Items whose name the prompt mentions — scanned once, reused for reference
  // enrichment below and for post-generation portrait tagging.
  const mentionedItems = chatId ? itemsMentionedIn(chatId, body.prompt) : [];

  // (2) Recurring named items: attach stored item portraits referenced by name.
  for (const itemAttachment of recurringItemReferences(mentionedItems)) {
    pushUniqueReference(references, inlineLocalReference(attachmentToReference(itemAttachment)));
    if (references.length >= MAX_IMAGE_REFERENCES) {
      break;
    }
  }

  const boundedReferences = references.slice(0, MAX_IMAGE_REFERENCES);
  const dimensions = dimensionsForImage(body.mode, body.aspect);

  const result = await callFluxWorker({
    prompt: body.prompt,
    mode: body.mode,
    backend: resolveImageBackend(body.backend),
    aspect: body.aspect,
    width: dimensions.width,
    height: dimensions.height,
    seed: body.seed,
    references: boundedReferences.map((reference) => ({
      name: reference.name,
      dataUrl: reference.dataUrl,
      url: reference.url,
    })),
  });
  if (!result.ok) {
    return result.response;
  }
  const generatedImage = result.image;

  if (body.messageId) {
    updateMessageGeneratedImage(body.messageId, generatedImage);
  }

  // Persist this scene as the hero's evolving reference, and tag any recurring
  // named item with its portrait, so the next generation reuses both. Purely
  // additive bookkeeping — never block returning the image on it.
  if (chatId && generatedImage?.url) {
    if (heroInScene && hero) {
      try {
        setCharacterReference(chatId, hero.id, {
          id: generatedImage.id || `ref-${hero.id}`,
          name: `${hero.name} (scene reference)`,
          type: "image/png",
          url: generatedImage.url,
        });
      } catch {
        // ignore — reference refresh is best-effort
      }
    }
    for (const item of mentionedItems) {
      try {
        setItemImage(chatId, { name: item.name.trim() }, generatedImage.url);
      } catch {
        // ignore — item image tagging is best-effort
      }
    }
  }

  return Response.json(generatedImage);
}
