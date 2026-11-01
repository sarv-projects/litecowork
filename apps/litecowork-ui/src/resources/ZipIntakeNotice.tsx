import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { decodeZipIntakeReadiness, type ZipIntakeReadiness } from "./zip-intake-api";
import "./zip-intake-notice.css";

/**
 * Honest resource-intake state while the isolated ZIP provider is unqualified.
 * Mount in the Workspace Library next to the ordinary file intake controls.
 */
export function ZipIntakeNotice({ workspaceId, operatorReady }: { workspaceId: string; operatorReady: boolean }) {
  const [readiness, setReadiness] = useState<ZipIntakeReadiness | null>(null);
  const [statusUnavailable, setStatusUnavailable] = useState(false);

  useEffect(() => {
    let active = true;
    setReadiness(null);
    setStatusUnavailable(false);
    if (!operatorReady || !workspaceId) {
      setStatusUnavailable(true);
      return () => { active = false; };
    }
    void invoke<unknown>("get_zip_intake_readiness", { workspaceId })
      .then((value) => {
        if (active) setReadiness(decodeZipIntakeReadiness(value));
      })
      .catch(() => {
        if (active) setStatusUnavailable(true);
      });
    return () => { active = false; };
  }, [workspaceId, operatorReady]);

  const title = readiness
    ? "ZIP extraction is unavailable"
    : statusUnavailable
      ? "ZIP extraction status unavailable"
      : "Checking ZIP extraction status…";
  return (
    <aside className="zip-intake-notice" aria-label="ZIP archive support" aria-live="polite">
      <strong>{title}</strong>
      <p>
        {readiness?.reason ?? (statusUnavailable
          ? "The local Runtime could not confirm extraction readiness. ZIPs remain opaque Resources."
          : "Checking local extraction readiness… ZIPs remain opaque Resources.")}
        {" "}ZIP archives can be saved intact; their contents are not unpacked, indexed, or attached to an agent.
      </p>
    </aside>
  );
}
