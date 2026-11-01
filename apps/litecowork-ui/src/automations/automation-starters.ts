export type AutomationStarterId = "coding-review" | "cowork-brief";
export type AutomationStarterCategory = "CODING" | "COWORK";

export interface AutomationStarterRoutineDraft {
  name: string;
  objective: string;
  instructions: string;
}

export interface AutomationStarter {
  id: AutomationStarterId;
  category: AutomationStarterCategory;
  categoryLabel: string;
  title: string;
  summary: string;
  requirements: readonly string[];
  expectedOutput: string;
  routineDraft: AutomationStarterRoutineDraft;
  automationDraft: {
    name: string;
    triggerKind: "SCHEDULE";
    cron: string;
  };
}

export interface AutomationTemplateFlow {
  workspaceId: string;
  starterId: AutomationStarterId;
  routineId: string | null;
  routineRevision: number | null;
}

export const AUTOMATION_STARTERS: readonly AutomationStarter[] = [
  {
    id: "coding-review",
    category: "CODING",
    categoryLabel: "Coding",
    title: "Weekday codebase review",
    summary: "Review available project materials and recent changes, then prepare an evidence-backed review Task.",
    requirements: [
      "A Workspace with the project materials you want reviewed.",
      "Review the local time zone and weekday schedule before saving.",
      "A saved Automation remains paused until you explicitly enable it.",
    ],
    expectedOutput: "Each due occurrence creates one READY Task. This build does not start planning or agent execution.",
    routineDraft: {
      name: "Weekday codebase review",
      objective: "Prepare an evidence-backed review of the selected Workspace project and its recent changes.",
      instructions: "Read repository-local guidance first. Review only project materials available in the selected Workspace. Verify each finding against a precise file and relevant source. Prioritize correctness, safety, maintainability, and user impact; distinguish confirmed defects from questions; propose the smallest safe next step. Do not edit files, run destructive commands, commit, publish, or send data externally. If the project or recent changes are unavailable, report that limitation instead of implying a review was completed.",
    },
    automationDraft: {
      name: "Weekday codebase review",
      triggerKind: "SCHEDULE",
      cron: "0 9 * * 1-5",
    },
  },
  {
    id: "cowork-brief",
    category: "COWORK",
    categoryLabel: "Cowork",
    title: "Weekly project briefing",
    summary: "Prepare a concise, source-grounded briefing from the resources in a Workspace.",
    requirements: [
      "A Workspace containing the project resources to summarize.",
      "Review the local time zone and weekly schedule before saving.",
      "A saved Automation remains paused until you explicitly enable it.",
    ],
    expectedOutput: "Each due occurrence creates one READY Task. This build does not start planning or agent execution.",
    routineDraft: {
      name: "Weekly project briefing",
      objective: "Prepare a concise, source-grounded update for the selected Workspace.",
      instructions: "Use only resources explicitly available in the selected Workspace. Summarize completed work, decisions, blockers, risks, dependencies, and next actions. Support claims with the exact source document or Resource revision; mark uncertainty, stale material, and missing context. Separate observed facts from inference. Do not edit source material or send or publish the briefing. If the available sources are incomplete, state what is missing and prepare only the supported portion.",
    },
    automationDraft: {
      name: "Weekly project briefing",
      triggerKind: "SCHEDULE",
      cron: "0 9 * * 1",
    },
  },
] as const;

export function getAutomationStarter(id: string): AutomationStarter | null {
  return AUTOMATION_STARTERS.find(starter => starter.id === id) ?? null;
}

export function routineDraftFromStarter(id: string): AutomationStarterRoutineDraft | null {
  const starter = getAutomationStarter(id);
  return starter ? { ...starter.routineDraft } : null;
}
