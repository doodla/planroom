/**
 * The mod's one section of the composed system prompt (architecture.md, "Prompt section"; DR-20):
 * the five ideas, which tool serves what, and to pass `base` from the `seq` read. It takes no
 * argument, so it is byte-identical across turns and states and stays in the cached prefix;
 * anything that changes by turn goes through `prompt.submit`, the band or the status line.
 */

import { MOD_STRINGS } from "@planroom/contract";

/** One section of the system prompt, as `prompt.compose` answers it. */
export interface PromptSection {
  id: string;
  text: string;
  scope: "shared" | "session";
}

/** The section's stable id, by the host's `<plugin>:<name>` convention. */
export const SECTION_ID = "planroom:map";

/** The mod's section: the same bytes in every state. */
export function promptSection(): PromptSection {
  return {
    id: SECTION_ID,
    text: MOD_STRINGS.prompt_section,
    scope: "session",
  };
}

/** `sections` with the mod's section appended once, last, in place of any earlier copy. */
export function composeSections<S extends PromptSection>(
  sections: readonly S[],
): (S | PromptSection)[] {
  return [...sections.filter((s) => s.id !== SECTION_ID), promptSection()];
}
