export type AgentCandidateCatalogStage = "PROFILE_PROBE_IMPLEMENTED" | "CANDIDATE_ONLY";

export type AgentCandidate = {
  id: string;
  displayName: string;
  candidatePath: string;
  catalogStage: AgentCandidateCatalogStage;
  profileProviderKey: "CODEX" | "OPENCODE" | null;
  installationAgentId: string | null;
  sessionModuleStatus: "NOT_IMPLEMENTED";
  sessionHandshakeStatus: "NOT_TESTED";
  qualifiedSupportStatus: "NOT_QUALIFIED";
};

function candidate(
  id: string,
  displayName: string,
  candidatePath: string,
  profileProviderKey: AgentCandidate["profileProviderKey"] = null,
  installationAgentId: string | null = null,
): AgentCandidate {
  return {
    id,
    displayName,
    candidatePath,
    catalogStage: profileProviderKey ? "PROFILE_PROBE_IMPLEMENTED" : "CANDIDATE_ONLY",
    profileProviderKey,
    installationAgentId,
    sessionModuleStatus: "NOT_IMPLEMENTED",
    sessionHandshakeStatus: "NOT_TESTED",
    qualifiedSupportStatus: "NOT_QUALIFIED",
  };
}

/**
 * Compiled product candidates, independent of ACP Registry availability and
 * local installation. These entries are not runnable adapters or support claims.
 */
export const AGENT_CANDIDATES: readonly AgentCandidate[] = [
  candidate("claude-code", "Claude Code", "Native CLI profile candidate; ACP module not established"),
  candidate("codex-cli", "Codex CLI", "Codex App Server profile probe", "CODEX", "codex-cli"),
  candidate("opencode", "OpenCode", "OpenCode profile probe; session adapter not qualified", "OPENCODE", "opencode"),
  candidate("cline-cli", "Cline CLI", "Native ACP candidate", null, "cline-cli"),
  candidate("gemini-cli", "Gemini CLI", "Native ACP candidate"),
  candidate("github-copilot-cli", "GitHub Copilot CLI", "Native ACP candidate; public preview"),
  candidate("qwen-code", "Qwen Code", "Native ACP candidate"),
  candidate("kiro-cli", "Kiro CLI", "Native ACP candidate"),
  candidate("goose", "Goose", "Native ACP candidate"),
  candidate("mistral-vibe", "Mistral Vibe", "Native ACP candidate"),
  candidate("cursor-cli", "Cursor CLI", "Native ACP candidate"),
  candidate("kimi-code-cli", "Kimi Code CLI", "Native ACP candidate"),
  candidate("jetbrains-junie-cli", "JetBrains Junie CLI", "Native ACP candidate"),
  candidate("factory-droid", "Factory Droid", "Launch contract unresolved"),
  candidate("auggie-cli", "Auggie CLI", "Native ACP candidate"),
];
