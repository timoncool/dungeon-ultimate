import { z } from "zod";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import {
  getActiveScene,
  getCharacterReference,
  getHeroCharacter,
  getMessageContext,
  getScene,
  listItems,
  normalizeLocation,
  recordSceneImage,
  setCharacterReference,
  setItemImage,
  updateMessageGeneratedImage,
} from "@/lib/db";
import { callFluxWorker, resolveImageBackend } from "@/lib/flux-worker";
import { applyEditContinuity, dimensionsForImage } from "@/lib/story-prompt";
import type { Item } from "@/lib/rpg/types";
import type { Attachment, GeneratedImage } from "@/lib/types";

export const runtime = "nodejs";

// Total references sent to the worker (character portrait(s) + the scene
// continuity image + a recurring item portrait). FLUX.2 Klein's strongest regime
// is 2-3 references; character first, then scene, then item.
const MAX_IMAGE_REFERENCES = 3;
// Bound iterative img2img drift: after this many consecutive edits of a location
// the engine re-anchors with a clean establishing render instead of editing on.
const MAX_EDIT_HOPS = 6;

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

  // Recover scene/img2img context from the message this image belongs to.
  // Best-effort: a missing/legacy message just means no server-side enrichment.
  const context = body.messageId ? getMessageContext(body.messageId) : null;
  const chatId = context?.chatId;
  const hero = chatId ? getHeroCharacter(chatId) : null;
  // Characters in this shot: the narrator's saved IDs, or — for an unattributed
  // scene — the protagonist, the default subject of the story.
  let sceneCharacterIds = context?.imageRequest?.characterIds ?? [];
  if (chatId && hero && sceneCharacterIds.length === 0) {
    sceneCharacterIds = [hero.id];
  }

  // --- Edit-vs-fresh decision against the chat's scene state -----------------
  // The narrator labels each shot with a stable `location` plus a `sameLocation`
  // hint. If this shot continues the active scene (same place, drift not yet
  // capped) the previous image is evolved; returning to a known place re-derives
  // from that place's anchor; a brand-new place is a fresh establishing shot.
  const requestedLocation = normalizeLocation(context?.imageRequest?.location);
  const sameLocationHint = context?.imageRequest?.sameLocation === true;
  let editFrom: Attachment | null = null;
  let willEdit = false;
  let sceneLocationKey = "";
  let priorHops = 0;

  if (chatId) {
    const active = getActiveScene(chatId);
    const continuesActive =
      !!active &&
      ((!!requestedLocation && active.location === requestedLocation) ||
        (sameLocationHint && !!(active.last || active.anchor)));

    if (continuesActive && active) {
      sceneLocationKey = active.location;
      priorHops = active.hops;
      const priorImage = active.last ?? active.anchor;
      if (priorImage && active.hops < MAX_EDIT_HOPS) {
        editFrom = priorImage;
        willEdit = true;
      }
      // hop cap reached -> fall through to a fresh re-anchor of the same place
    } else if (requestedLocation) {
      sceneLocationKey = requestedLocation;
      const past = getScene(chatId, requestedLocation);
      if (past?.anchor) {
        // Revisit: re-establish from the location's anchor for visual continuity.
        editFrom = past.anchor;
        willEdit = true;
      }
      // else brand-new location -> fresh establishing shot
    }
    // no location label and not the active scene -> untracked one-off fresh image
  }

  // --- Build the reference set (character -> scene -> item -> client) --------
  const references: ImageReference[] = [];

  // (1) Character references first: identity is the highest-drift risk, so the
  // saved portrait / evolving look leads. On a re-encounter this is the stored
  // userpic being reused. Hero is already in sceneCharacterIds.
  if (chatId) {
    for (const id of sceneCharacterIds) {
      if (references.length >= MAX_IMAGE_REFERENCES) {
        break;
      }
      const characterReference = getCharacterReference(chatId, id);
      if (characterReference) {
        pushUniqueReference(
          references,
          inlineLocalReference({
            ...attachmentToReference(characterReference),
            id: `character-${id}`,
          }),
        );
      }
    }
  }

  // (2) Scene continuity reference: the prior image of this location, so the edit
  // evolves the established scene instead of redrawing it.
  if (willEdit && editFrom) {
    pushUniqueReference(
      references,
      inlineLocalReference({ ...attachmentToReference(editFrom), id: `scene-${sceneLocationKey}` }),
    );
  }

  // (3) Recurring named item portraits, scanned once and reused for tagging below.
  const mentionedItems = chatId ? itemsMentionedIn(chatId, body.prompt) : [];
  for (const itemAttachment of recurringItemReferences(mentionedItems)) {
    if (references.length >= MAX_IMAGE_REFERENCES) {
      break;
    }
    pushUniqueReference(references, inlineLocalReference(attachmentToReference(itemAttachment)));
  }

  // (4) Fill any remaining slots with client-provided references (this-turn
  // attachments the player added).
  for (const clientReference of body.references) {
    if (references.length >= MAX_IMAGE_REFERENCES) {
      break;
    }
    pushUniqueReference(references, inlineLocalReference(clientReference));
  }

  const boundedReferences = references.slice(0, MAX_IMAGE_REFERENCES);
  const dimensions = dimensionsForImage(body.mode, body.aspect);
  // An edit prepends the continuity-preservation directive so the model keeps the
  // referenced scene and changes only what the passage added.
  const finalPrompt = willEdit ? applyEditContinuity(body.prompt) : body.prompt;

  console.info(
    `[images] ${willEdit ? `edit(hop ${priorHops + 1})` : "fresh"} chat=${
      chatId?.slice(0, 8) ?? "-"
    } loc="${sceneLocationKey || "-"}" shot=${context?.imageRequest?.shot ?? "-"} refs=[${
      boundedReferences.map((reference) => reference.name).join(" + ") || "none"
    }] mode=${body.mode}`,
  );
  const startedAt = Date.now();

  const result = await callFluxWorker({
    prompt: finalPrompt,
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
    console.warn(
      `[images] failed chat=${chatId?.slice(0, 8) ?? "-"} loc="${sceneLocationKey || "-"}"`,
    );
    return result.response;
  }

  const generatedImage: GeneratedImage = {
    ...result.image,
    ...(sceneLocationKey ? { sceneLocation: sceneLocationKey } : {}),
    ...(willEdit && editFrom ? { editedFrom: editFrom.url } : {}),
  };
  console.info(
    `[images] done chat=${chatId?.slice(0, 8) ?? "-"} url=${generatedImage.url} ${
      Math.round((Date.now() - startedAt) / 100) / 10
    }s`,
  );

  if (body.messageId) {
    updateMessageGeneratedImage(body.messageId, generatedImage);
  }

  // Additive continuity bookkeeping — never block returning the image on it.
  if (chatId && generatedImage.url) {
    const sceneAttachment: Attachment = {
      id: generatedImage.id || `scene-${sceneLocationKey || "img"}`,
      name: sceneLocationKey ? `scene: ${sceneLocationKey}` : "scene reference",
      type: "image/png",
      url: generatedImage.url,
    };

    // Record this image for its location (anchor on a fresh/establishing shot,
    // edit otherwise) so the next turn in this place can evolve it.
    if (sceneLocationKey) {
      try {
        recordSceneImage(chatId, sceneLocationKey, sceneAttachment, { anchor: !willEdit });
      } catch {
        // best-effort
      }
    }

    // Refresh evolving character references so a re-encounter reuses the look:
    // the hero always; an NPC only when they are the sole subject of the shot, so
    // a group scene never overwrites a clean single-character portrait.
    const soloSubject = sceneCharacterIds.length === 1;
    for (const id of sceneCharacterIds) {
      if (hero?.id === id || soloSubject) {
        try {
          setCharacterReference(chatId, id, {
            id: generatedImage.id || `ref-${id}`,
            name: `character ${id} (scene reference)`,
            type: "image/png",
            url: generatedImage.url,
          });
        } catch {
          // best-effort
        }
      }
    }

    // Tag recurring named items with this portrait for consistent reuse.
    for (const item of mentionedItems) {
      try {
        setItemImage(chatId, { name: item.name.trim() }, generatedImage.url);
      } catch {
        // best-effort
      }
    }
  }

  return Response.json(generatedImage);
}
