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
import { serverEnv } from "@/lib/server-env";
import { dimensionsForImage } from "@/lib/story-prompt";
import type { Attachment, GeneratedImage } from "@/lib/types";

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

// Stored items, with a generated portrait, whose name the prompt mentions — so a
// recurring named item is illustrated consistently. Longer names win first.
function recurringItemReferences(chatId: string, prompt: string): Attachment[] {
  const haystack = prompt.toLowerCase();
  const items = listItems(chatId)
    .filter((item) => item.imageUrl && item.name.trim())
    .sort((a, b) => b.name.length - a.name.length);
  const matched: Attachment[] = [];
  for (const item of items) {
    if (!mentionsName(haystack, item.name)) {
      continue;
    }
    matched.push({
      id: `item-${item.id}`,
      name: item.name,
      type: "image/png",
      url: item.imageUrl as string,
    });
  }
  return matched;
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

  // (2) Recurring named items: attach stored item portraits referenced by name.
  if (chatId) {
    for (const itemAttachment of recurringItemReferences(chatId, body.prompt)) {
      pushUniqueReference(references, inlineLocalReference(attachmentToReference(itemAttachment)));
      if (references.length >= MAX_IMAGE_REFERENCES) {
        break;
      }
    }
  }

  const boundedReferences = references.slice(0, MAX_IMAGE_REFERENCES);
  const workerUrl = serverEnv("FLUX_WORKER_URL", "http://127.0.0.1:7869");
  const dimensions = dimensionsForImage(body.mode, body.aspect);
  // MFLUX is Apple-Silicon only; on Windows/Linux use the uncensored FLUX.2-klein CUDA backend.
  // Both the Mac default (mflux-hs) and the stock censored SDNQ (sdnq-hs) resolve to it so this
  // NSFW app always runs the uncensored text encoder unless flux-uncensored is sent explicitly.
  const defaultBackend = serverEnv("IMAGE_SERVER_DEFAULT_BACKEND", "flux-uncensored");
  const backend =
    body.backend === "mflux-hs" || body.backend === "sdnq-hs" ? defaultBackend : body.backend;

  try {
    const upstream = await fetch(`${workerUrl.replace(/\/$/, "")}/generate`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        prompt: body.prompt,
        mode: body.mode,
        backend,
        aspect: body.aspect,
        width: dimensions.width,
        height: dimensions.height,
        steps: 4,
        guidance: 0.0,
        seed: body.seed,
        references: boundedReferences.map((reference) => ({
          name: reference.name,
          dataUrl: reference.dataUrl,
          url: reference.url,
        })),
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
      for (const itemName of itemNamesInPrompt(chatId, body.prompt)) {
        try {
          setItemImage(chatId, { name: itemName }, generatedImage.url);
        } catch {
          // ignore — item image tagging is best-effort
        }
      }
    }

    return Response.json(generatedImage);
  } catch (error) {
    return Response.json(
      {
        error: "Flux worker is not running.",
        detail: error instanceof Error ? error.message : String(error),
        expected: "Откройте Images и нажмите Start, или запустите npm run image:server из папки Open Dungeon.",
      },
      { status: 503 },
    );
  }
}

// Names of stored items mentioned in the prompt. Used to attach a freshly
// generated portrait to a newly illustrated item drop (reused on next mention).
function itemNamesInPrompt(chatId: string, prompt: string): string[] {
  const haystack = prompt.toLowerCase();
  const names: string[] = [];
  for (const item of listItems(chatId)) {
    const name = item.name.trim();
    if (name && mentionsName(haystack, name)) {
      names.push(name);
    }
  }
  return names;
}
