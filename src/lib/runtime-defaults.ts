import { DEFAULT_STORY_SETTINGS } from "@/lib/defaults";
import { serverEnv } from "@/lib/server-env";
import {
  isLocalTextModelId,
  isTextProvider,
} from "@/lib/text-models";
import type { StorySettings } from "@/lib/types";

function clean(value: string) {
  return value.trim();
}

export function configuredDefaultStorySettings(): StorySettings {
  const customBaseUrl = clean(serverEnv("OPENAI_COMPAT_BASE_URL"));
  const openRouterDefaultModel = /(^|\.)openrouter\.ai/i.test(customBaseUrl)
    ? clean(serverEnv("OPENROUTER_MODEL", "google/gemini-3.5-flash"))
    : "";
  const customModel = clean(serverEnv("OPENAI_COMPAT_MODEL")) || openRouterDefaultModel;
  const requestedProvider = clean(serverEnv("DEFAULT_TEXT_PROVIDER"));
  const textProvider = isTextProvider(requestedProvider)
    ? requestedProvider
    : customBaseUrl
      ? "custom"
      : DEFAULT_STORY_SETTINGS.textProvider;
  const requestedLocalModel = clean(serverEnv("LOCAL_TEXT_MODEL"));
  const localTextModel = isLocalTextModelId(requestedLocalModel)
    ? requestedLocalModel
    : DEFAULT_STORY_SETTINGS.localTextModel;

  return {
    ...DEFAULT_STORY_SETTINGS,
    textProvider,
    localTextModel,
    customBaseUrl,
    customModel,
    customApiKey: "",
  };
}
