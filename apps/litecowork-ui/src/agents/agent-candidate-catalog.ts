export type AgentCandidateCatalogStage = "PROFILE_PROBE_IMPLEMENTED" | "INSTALLATION_PROBE_IMPLEMENTED" | "CANDIDATE_ONLY";

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
    catalogStage: profileProviderKey
      ? "PROFILE_PROBE_IMPLEMENTED"
      : installationAgentId ? "INSTALLATION_PROBE_IMPLEMENTED" : "CANDIDATE_ONLY",
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
  candidate("claude-code", "Claude Code", "Bounded CLI installation probe; profile module not established", null, "claude-code"),
  candidate("codex-cli", "Codex CLI", "Codex App Server profile probe", "CODEX", "codex-cli"),
  candidate("opencode", "OpenCode", "OpenCode profile probe; session adapter not qualified", "OPENCODE", "opencode"),
  candidate("cline-cli", "Cline CLI", "Native ACP candidate", null, "cline-cli"),
  candidate("gemini-cli", "Gemini CLI", "Bounded CLI installation probe; ACP profile module not established", null, "gemini-cli"),
  candidate("github-copilot-cli", "GitHub Copilot CLI", "Bounded CLI installation probe; native session module not established", null, "github-copilot-cli"),
  candidate("qwen-code", "Qwen Code", "Bounded CLI installation probe; ACP profile module not established", null, "qwen-code"),
  candidate("kiro-cli", "Kiro CLI", "Bounded CLI installation probe; ACP profile module not established", null, "kiro-cli"),
  candidate("goose", "Goose", "Bounded CLI installation probe; ACP profile module not established", null, "goose"),
  candidate("mistral-vibe", "Mistral Vibe", "Bounded CLI installation probe; profile module not established", null, "mistral-vibe"),
  candidate("cursor-cli", "Cursor CLI", "Bounded CLI installation probe; profile module not established", null, "cursor-cli"),
  candidate("kimi-code-cli", "Kimi Code CLI", "Bounded CLI installation probe; native session module not established", null, "kimi-code-cli"),
  candidate("jetbrains-junie-cli", "JetBrains Junie CLI", "Bounded CLI installation probe; ACP profile module not established", null, "jetbrains-junie-cli"),
  candidate("factory-droid", "Factory Droid", "Bounded CLI installation probe; native session module not established", null, "factory-droid"),
  candidate("auggie-cli", "Auggie CLI", "Bounded CLI installation probe; native session module not established", null, "auggie-cli"),
];
