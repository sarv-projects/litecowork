import test from "node:test";
import assert from "node:assert/strict";
import {
  isSensitiveOrGenerated,
  resourceDisplayName,
  resourceFolderRelativePath,
  validateResourceFileSelection,
} from "../src/resources/resource-intake.ts";

function file(name: string, size = 1, webkitRelativePath = "") {
  return { name, size, webkitRelativePath };
}

test("folder import preserves nested relative names without normalizing unsafe separators", () => {
  const selected = file("notes.md", 12, "project/docs/notes.md");
  assert.equal(resourceFolderRelativePath(selected), "project/docs/notes.md");
  assert.equal(resourceDisplayName(selected), "project/docs/notes.md");
  assert.throws(() => resourceFolderRelativePath(file("secret.txt", 1, "project\\.env\\secret.txt")), /invalid relative path/);
});

test("folder import rejects absolute, traversal, drive, control, and oversized relative paths", () => {
  for (const path of [
    "/project/file.txt",
    "project/../outside.txt",
    "C:/project/file.txt",
    "project/file\u0000.txt",
    `${"segment/".repeat(128)}file.txt`,
    `project/${"x".repeat(256)}.txt`,
  ]) {
    assert.throws(() => resourceFolderRelativePath(file("file.txt", 1, path)), /invalid relative path/, path);
  }
});

test("secret and generated paths are excluded from a mixed folder selection", () => {
  for (const selected of [
    file(".env"),
    file(".env.production"),
    file("id_ed25519", 1, "project/.ssh/id_ed25519"),
    file("credentials.json", 1, "project/.aws/credentials.json"),
    file("cache.bin", 1, "project/node_modules/cache.bin"),
    file("result.o", 1, "project/target/debug/result.o"),
  ]) {
    assert.equal(isSensitiveOrGenerated(selected), true, selected.webkitRelativePath || selected.name);
  }
  assert.equal(isSensitiveOrGenerated(file("README.md", 4, "project/docs/README.md")), false);
});

test("Resource intake enforces file-count, aggregate-byte, and per-file bounds", () => {
  const maxFiles = Array.from({ length: 100 }, (_, index) => file(`file-${index}.txt`, 1));
  assert.equal(validateResourceFileSelection(maxFiles), null);
  assert.match(validateResourceFileSelection([...maxFiles, file("extra.txt")]) ?? "", /up to 100 files/);
  assert.match(validateResourceFileSelection([file("large.txt", 100 * 1024 * 1024 + 1)]) ?? "", /Each file is limited to 100 MiB/);
  assert.match(validateResourceFileSelection([file("one.txt", 60 * 1024 * 1024), file("two.txt", 40 * 1024 * 1024 + 1)]) ?? "", /100 MiB total/);
  assert.equal(validateResourceFileSelection([file("exact.bin", 100 * 1024 * 1024)]), null);
});
