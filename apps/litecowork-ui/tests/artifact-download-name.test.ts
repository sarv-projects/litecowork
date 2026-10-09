import assert from "node:assert/strict";
import test from "node:test";
import { artifactDownloadFileName } from "../src/artifacts/artifact-download-name.ts";

test("places the immutable version suffix before a recognized filename extension", () => {
  assert.equal(artifactDownloadFileName("quarterly-report.pdf", 7), "quarterly-report-v7.pdf");
});

test("keeps names without an extension and treats dotfiles as extensionless", () => {
  assert.equal(artifactDownloadFileName("analysis", 2), "analysis-v2");
  assert.equal(artifactDownloadFileName(".env", 3), ".env-v3");
});

test("removes path separators and control characters from the download name", () => {
  assert.equal(artifactDownloadFileName("../draft\\final\n.pdf", 1), ".._draft_final_-v1.pdf");
});

test("uses a safe fallback when the display name is empty after sanitization", () => {
  assert.equal(artifactDownloadFileName("/\\\u0000", 4), "artifact-v4");
});

test("rejects invalid immutable version numbers", () => {
  assert.throws(() => artifactDownloadFileName("report.md", 0), /positive safe integer/);
  assert.throws(() => artifactDownloadFileName("report.md", Number.MAX_SAFE_INTEGER + 1), /positive safe integer/);
});
