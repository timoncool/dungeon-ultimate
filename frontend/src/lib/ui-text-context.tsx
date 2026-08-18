import { createContext, useContext, useMemo, type ReactNode } from "react";

import { uiText, type UiText } from "@/lib/ui-text";
import type { Language } from "@/lib/types";

/// Язык интерфейса берётся из языка истории: игрок выбирает его один раз, и странно, когда
/// рассказ идёт по-японски, а кнопки — по-русски.
const UiTextContext = createContext<UiText>(uiText("ru"));

export function UiTextProvider({
  language,
  children,
}: {
  language: Language | undefined;
  children: ReactNode;
}) {
  const value = useMemo(() => uiText(language), [language]);
  return <UiTextContext.Provider value={value}>{children}</UiTextContext.Provider>;
}

/// Подписи интерфейса. Вне провайдера отдаёт русские — чтобы компонент не падал в тестах.
export function useUi(): UiText {
  return useContext(UiTextContext);
}
