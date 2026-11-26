export type CalcFailureReason =
  | "DIVIDE_BY_ZERO"
  | "NON_FINITE"
  | "INVALID_AST"
  | "INVALID_BOUNDS"
  | "MISSING_INPUT"
  | "INVALID_INPUTS";

export type CalcEvaluation =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly reason: CalcFailureReason };

type NormalizedNode = {
  kind: "LITERAL" | "VARIABLE" | "BINARY_OP" | "UNARY_OP" | "FUNCTION";
  value?: number;
  inputId?: string;
  operator?: "+" | "-" | "*" | "/" | "%";
  function?: "MIN" | "MAX" | "ROUND" | "ABS" | "CLAMP";
  children: NormalizedNode[];
};

type NodeFrame = {
  raw: unknown;
  depth: number;
  parent: NormalizedNode | null;
  childIndex: number;
};

const MAX_AST_NODES = 64;
const MAX_AST_DEPTH = 12;
const INPUT_ID = /^[A-Za-z][A-Za-z0-9_-]{0,63}$/;
const BINARY_OPERATORS = new Set(["+", "-", "*", "/", "%"]);
const FUNCTIONS = new Set(["MIN", "MAX", "ROUND", "ABS", "CLAMP"]);

function invalidAst(): CalcEvaluation {
  return { ok: false, reason: "INVALID_AST" };
}

function readDataObject(value: unknown): Record<string, unknown> | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) return null;
  const keys = Reflect.ownKeys(value);

  const result: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const key of keys) {
    if (typeof key !== "string") return null;
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return null;
    result[key] = descriptor.value;
  }
  return result;
}

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value);
  return actual.length === expected.length && expected.every((key) => Object.hasOwn(value, key));
}

function readBoundedArray(value: unknown, maximumLength: number): unknown[] | null {
  if (!Array.isArray(value)) return null;
  const lengthDescriptor = Object.getOwnPropertyDescriptor(value, "length");
  if (!lengthDescriptor || !("value" in lengthDescriptor) || !Number.isSafeInteger(lengthDescriptor.value)) return null;
  const length = lengthDescriptor.value as number;
  if (length < 0 || length > maximumLength) return null;
  const keys = Reflect.ownKeys(value);
  if (keys.length !== length + 1 || !keys.includes("length")) return null;
  for (const key of keys) {
    if (key === "length") continue;
    if (typeof key !== "string" || !/^(0|[1-9]\d*)$/.test(key) || Number(key) >= length) return null;
  }
  const result: unknown[] = [];
  for (let index = 0; index < length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) return null;
    result.push(descriptor.value);
  }
  return result;
}

function snapshotAst(root: unknown): NormalizedNode | null {
  const visited = new WeakSet<object>();
  const pending: NodeFrame[] = [{ raw: root, depth: 0, parent: null, childIndex: -1 }];
  const nodes: NormalizedNode[] = [];
  let rootNode: NormalizedNode | null = null;
  let nodeCount = 0;

  while (pending.length > 0) {
    const frame = pending.pop()!;
    if (++nodeCount > MAX_AST_NODES || frame.depth > MAX_AST_DEPTH
      || frame.raw === null || typeof frame.raw !== "object" || Array.isArray(frame.raw)
      || visited.has(frame.raw)) return null;
    visited.add(frame.raw);

    const fields = readDataObject(frame.raw);
    if (!fields || typeof fields.kind !== "string") return null;
    const kind = fields.kind;
    let node: NormalizedNode;
    let childRaws: unknown[] = [];

    if (kind === "LITERAL") {
      if (!hasExactKeys(fields, ["kind", "value"]) || typeof fields.value !== "number" || !Number.isFinite(fields.value)) return null;
      node = { kind, value: fields.value, children: [] };
    } else if (kind === "VARIABLE") {
      if (!hasExactKeys(fields, ["kind", "input_id"]) || typeof fields.input_id !== "string" || !INPUT_ID.test(fields.input_id)) return null;
      node = { kind, inputId: fields.input_id, children: [] };
    } else if (kind === "BINARY_OP") {
      if (!hasExactKeys(fields, ["kind", "operator", "left", "right"])
        || typeof fields.operator !== "string" || !BINARY_OPERATORS.has(fields.operator)) return null;
      node = { kind, operator: fields.operator as NormalizedNode["operator"], children: new Array<NormalizedNode>(2) };
      childRaws = [fields.left, fields.right];
    } else if (kind === "UNARY_OP") {
      if (!hasExactKeys(fields, ["kind", "operator", "operand"]) || fields.operator !== "-") return null;
      node = { kind, operator: "-", children: new Array<NormalizedNode>(1) };
      childRaws = [fields.operand];
    } else if (kind === "FUNCTION") {
      if (!hasExactKeys(fields, ["kind", "function", "arguments"])
        || typeof fields.function !== "string" || !FUNCTIONS.has(fields.function)) return null;
      const args = readBoundedArray(fields.arguments, 8);
      if (!args) return null;
      const arityValid = fields.function === "MIN" || fields.function === "MAX" ? args.length >= 2 && args.length <= 8
        : fields.function === "ROUND" ? args.length >= 1 && args.length <= 2
          : fields.function === "ABS" ? args.length === 1
            : fields.function === "CLAMP" ? args.length === 3 : false;
      if (!arityValid) return null;
      node = { kind, function: fields.function as NormalizedNode["function"], children: new Array<NormalizedNode>(args.length) };
      childRaws = args;
    } else {
      return null;
    }

    nodes.push(node);
    if (frame.parent) frame.parent.children[frame.childIndex] = node;
    else rootNode = node;
    for (let index = childRaws.length - 1; index >= 0; index -= 1) {
      pending.push({ raw: childRaws[index], depth: frame.depth + 1, parent: node, childIndex: index });
    }
  }

  if (!rootNode) return null;
  for (const node of nodes) {
    if (node.kind === "FUNCTION" && node.function === "ROUND" && node.children.length === 2) {
      const precision = node.children[1];
      if (precision.kind !== "LITERAL" || !Number.isInteger(precision.value)
        || precision.value! < -6 || precision.value! > 6) return null;
    }
  }
  return rootNode;
}

function snapshotInputValues(inputValues: ReadonlyMap<string, number>):
  | { readonly ok: true; readonly values: Map<string, number> }
  | { readonly ok: false; readonly reason: "NON_FINITE" | "INVALID_INPUTS" } {
  try {
    const values = new Map<string, number>();
    const entries = Map.prototype.entries.call(inputValues as Map<string, number>) as IterableIterator<[string, number]>;
    for (const [inputId, value] of entries) {
      if (typeof inputId !== "string" || !INPUT_ID.test(inputId) || typeof value !== "number") {
        return { ok: false, reason: "INVALID_INPUTS" };
      }
      if (!Number.isFinite(value)) return { ok: false, reason: "NON_FINITE" };
      values.set(inputId, value);
    }
    return { ok: true, values };
  } catch {
    return { ok: false, reason: "INVALID_INPUTS" };
  }
}

function evaluateNormalized(root: NormalizedNode, inputs: ReadonlyMap<string, number>): CalcEvaluation {
  const pending: Array<{ node: NormalizedNode; reduce: boolean }> = [{ node: root, reduce: false }];
  const values: number[] = [];

  while (pending.length > 0) {
    const frame = pending.pop()!;
    const { node } = frame;
    if (!frame.reduce) {
      if (node.kind === "LITERAL") {
        values.push(node.value!);
      } else if (node.kind === "VARIABLE") {
        const value = inputs.get(node.inputId!);
        if (value === undefined) return { ok: false, reason: "MISSING_INPUT" };
        values.push(value);
      } else {
        pending.push({ node, reduce: true });
        for (let index = node.children.length - 1; index >= 0; index -= 1) {
          pending.push({ node: node.children[index]!, reduce: false });
        }
      }
      continue;
    }

    const childStart = values.length - node.children.length;
    if (childStart < 0) return invalidAst();
    const args = values.slice(childStart);
    values.length = childStart;
    let result: number;

    if (node.kind === "BINARY_OP") {
      const left = args[0]!;
      const right = args[1]!;
      if ((node.operator === "/" || node.operator === "%") && right === 0) {
        return { ok: false, reason: "DIVIDE_BY_ZERO" };
      }
      switch (node.operator) {
        case "+": result = left + right; break;
        case "-": result = left - right; break;
        case "*": result = left * right; break;
        case "/": result = left / right; break;
        case "%": result = left % right; break;
        default: return invalidAst();
      }
    } else if (node.kind === "UNARY_OP") {
      result = -args[0]!;
    } else if (node.kind === "FUNCTION") {
      switch (node.function) {
        case "MIN":
          result = args[0]!;
          for (let index = 1; index < args.length; index += 1) result = Math.min(result, args[index]!);
          break;
        case "MAX":
          result = args[0]!;
          for (let index = 1; index < args.length; index += 1) result = Math.max(result, args[index]!);
          break;
        case "ABS": result = Math.abs(args[0]!); break;
        case "CLAMP":
          if (args[1]! > args[2]!) return { ok: false, reason: "INVALID_BOUNDS" };
          result = Math.min(Math.max(args[0]!, args[1]!), args[2]!);
          break;
        case "ROUND": {
          const precision = node.children.length === 2 ? node.children[1]!.value! : 0;
          const factor = 10 ** precision;
          const scaled = args[0]! * factor;
          if (!Number.isFinite(scaled)) return { ok: false, reason: "NON_FINITE" };
          const rounded = Math.sign(scaled) * Math.floor(Math.abs(scaled) + 0.5);
          result = rounded / factor;
          break;
        }
        default: return invalidAst();
      }
    } else {
      return invalidAst();
    }

    if (!Number.isFinite(result)) return { ok: false, reason: "NON_FINITE" };
    values.push(result);
  }

  return values.length === 1 && Number.isFinite(values[0])
    ? { ok: true, value: values[0]! }
    : invalidAst();
}

/** Evaluate only the closed CalcAst v1 profile; this function has no host side effects. */
export function evaluateCalc(ast: unknown, inputValues: ReadonlyMap<string, number>): CalcEvaluation {
  const inputSnapshot = snapshotInputValues(inputValues);
  if (!inputSnapshot.ok) return { ok: false, reason: inputSnapshot.reason };
  try {
    const normalized = snapshotAst(ast);
    if (!normalized) return invalidAst();
    return evaluateNormalized(normalized, inputSnapshot.values);
  } catch {
    // Reflection traps and malformed caller objects are data errors, never UI exceptions.
    return invalidAst();
  }
}
