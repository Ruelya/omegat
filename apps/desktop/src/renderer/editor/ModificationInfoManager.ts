/** Java `org.omegat.gui.editor.ModificationInfoManager`. */
export type ModificationInfo = { author: string; date: string; origin?: string };

export function formatModification(info: ModificationInfo, withDate = true): string {
  return withDate ? `${info.author} ${info.date}` : info.author;
}
