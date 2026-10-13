import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import openapiTS, { astToString } from "openapi-typescript";
import ts from "typescript";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, "../../..");
const input = path.join(repositoryRoot, "docs/schemas/operator-api.openapi.yaml");
const output = path.resolve(scriptDirectory, "../src/api/operator-api.ts");
let recursiveJsonSchemaCount = 0;

const ast = await openapiTS(pathToFileURL(input), {
  exportType: true,
  inject: "type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };",
  transform(_schema, options) {
    if (options.path === "#/components/schemas/JsonValue") {
      recursiveJsonSchemaCount += 1;
      return ts.factory.createTypeReferenceNode(ts.factory.createIdentifier("JsonValue"));
    }
  },
});

if (recursiveJsonSchemaCount !== 1) {
  throw new Error(`Expected one JsonValue schema transformation, found ${recursiveJsonSchemaCount}.`);
}

const generated = astToString(ast);
if (process.argv.includes("--check")) {
  const current = await readFile(output, "utf8").catch(() => null);
  if (current !== generated) {
    console.error("Generated Operator API types are stale; run `pnpm run api:generate`.");
    process.exitCode = 1;
  }
} else {
  await mkdir(path.dirname(output), { recursive: true });
  await writeFile(output, generated);
}
