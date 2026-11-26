import assert from "node:assert/strict";
import { test } from "node:test";

const reducer = await import("../src/presentation/calc-reducer.ts").catch(() => null);
const evaluateCalc = reducer?.evaluateCalc;

function evaluate(ast: unknown, inputValues: ReadonlyMap<string, number> = new Map()) {
  assert.equal(typeof evaluateCalc, "function", "calc-reducer exports evaluateCalc");
  return evaluateCalc!(ast, inputValues);
}

const literal = (value: number) => ({ kind: "LITERAL", value });
const variable = (input_id: string) => ({ kind: "VARIABLE", input_id });
const binary = (operator: string, left: unknown, right: unknown) => ({ kind: "BINARY_OP", operator, left, right });
const unary = (operand: unknown) => ({ kind: "UNARY_OP", operator: "-", operand });
const func = (name: string, ...arguments_: unknown[]) => ({ kind: "FUNCTION", function: name, arguments: arguments_ });

function expectValue(ast: unknown, value: number, inputs: ReadonlyMap<string, number> = new Map()) {
  assert.deepEqual(evaluate(ast, inputs), { ok: true, value });
}

function expectReason(ast: unknown, reason: string, inputs: ReadonlyMap<string, number> = new Map()) {
  assert.deepEqual(evaluate(ast, inputs), { ok: false, reason });
}

test("evaluates bounded arithmetic and variables without mutating the input map", () => {
  const inputs = new Map([["amount", 12], ["years", 4]]);
  const original = [...inputs.entries()];

  expectValue(binary("+", variable("amount"), variable("years")), 16, inputs);
  expectValue(binary("-", variable("amount"), variable("years")), 8, inputs);
  expectValue(binary("*", variable("amount"), variable("years")), 48, inputs);
  expectValue(binary("/", variable("amount"), variable("years")), 3, inputs);
  expectValue(binary("%", variable("amount"), literal(5)), 2, inputs);
  expectValue(unary(variable("amount")), -12, inputs);
  assert.deepEqual([...inputs.entries()], original);
});

test("supports only the registered MIN, MAX, ABS, CLAMP, and ROUND functions", () => {
  expectValue(func("MIN", literal(4), literal(-2), literal(8)), -2);
  expectValue(func("MAX", literal(4), literal(-2), literal(8)), 8);
  expectValue(func("ABS", literal(-9)), 9);
  expectValue(func("CLAMP", literal(12), literal(0), literal(10)), 10);
  expectValue(func("CLAMP", literal(-2), literal(0), literal(10)), 0);
  expectValue(func("CLAMP", literal(4), literal(0), literal(10)), 4);
});

test("ROUND matches CalcAst v1 ties-away-from-zero for both precision directions", () => {
  expectValue(func("ROUND", literal(1.25), literal(1)), 1.3);
  expectValue(func("ROUND", literal(-1.25), literal(1)), -1.3);
  expectValue(func("ROUND", literal(151), literal(-2)), 200);
  expectValue(func("ROUND", literal(-151), literal(-2)), -200);
  expectValue(func("ROUND", literal(1.5)), 2);
});

test("returns typed arithmetic failures for zero division, invalid bounds, and non-finite results", () => {
  expectReason(binary("/", literal(1), literal(0)), "DIVIDE_BY_ZERO");
  expectReason(binary("%", literal(1), literal(-0)), "DIVIDE_BY_ZERO");
  expectReason(func("CLAMP", literal(1), literal(5), literal(2)), "INVALID_BOUNDS");
  expectReason(binary("*", literal(1e308), literal(1e308)), "NON_FINITE");
});

test("rejects malformed ASTs and reports missing or non-finite input values without throwing", () => {
  expectReason({ kind: "SCRIPT", source: "1 + 1" }, "INVALID_AST");
  expectReason(binary("**", literal(2), literal(3)), "INVALID_AST");
  expectReason({ kind: "UNARY_OP", operator: "+", operand: literal(1) }, "INVALID_AST");
  expectReason(func("POW", literal(2), literal(3)), "INVALID_AST");
  expectReason(func("ABS", literal(1), literal(2)), "INVALID_AST");
  expectReason(variable("missing"), "MISSING_INPUT");
  expectReason(variable("amount"), "NON_FINITE", new Map([["amount", Number.POSITIVE_INFINITY]]));
  expectReason(variable("amount"), "INVALID_INPUTS", {
    [Symbol.iterator]() { throw new Error("hostile input map"); },
  } as unknown as ReadonlyMap<string, number>);
});

test("rejects accessors, cycles, excessive AST depth, and more than 64 nodes", () => {
  let getterCalled = false;
  const getterAst = { kind: "LITERAL" } as Record<string, unknown>;
  Object.defineProperty(getterAst, "value", { enumerable: true, get() { getterCalled = true; throw new Error("getter ran"); } });
  expectReason(getterAst, "INVALID_AST");
  assert.equal(getterCalled, false);

  let kindDescriptorReads = 0;
  const descriptorSnapshotProxy = new Proxy(literal(7), {
    getOwnPropertyDescriptor(target, key) {
      if (key === "kind") kindDescriptorReads += 1;
      return Reflect.getOwnPropertyDescriptor(target, key);
    },
  });
  expectValue(descriptorSnapshotProxy, 7);
  assert.equal(kindDescriptorReads, 1, "each AST property is read into the snapshot only once");

  const cyclic: Record<string, unknown> = { kind: "UNARY_OP", operator: "-" };
  cyclic.operand = cyclic;
  expectReason(cyclic, "INVALID_AST");

  const unaryChain = (count: number, value: unknown = literal(1)): unknown => {
    for (let index = 0; index < count; index += 1) value = unary(value);
    return value;
  };
  expectValue(unaryChain(12), 1);
  expectReason(unaryChain(13), "INVALID_AST");

  const balanced = (depth: number): unknown => depth === 0
    ? literal(1)
    : binary("+", balanced(depth - 1), balanced(depth - 1));
  expectValue(unary(balanced(5)), -32);
  expectReason(unary(unary(balanced(5))), "INVALID_AST");
});

function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0x1_0000_0000;
  };
}

function generatedAst(random: () => number, depth = 0): unknown {
  if (depth >= 4 || random() < 0.28) return random() < 0.5 ? literal(Math.round(random() * 20) - 10) : variable("x");
  const choice = Math.floor(random() * 8);
  if (choice === 0) return unary(generatedAst(random, depth + 1));
  if (choice === 1) return func("ABS", generatedAst(random, depth + 1));
  if (choice === 2) return func("ROUND", generatedAst(random, depth + 1), literal(Math.floor(random() * 7) - 3));
  if (choice === 3) return func("CLAMP", generatedAst(random, depth + 1), literal(-10), literal(10));
  if (choice === 4 || choice === 5) return func(choice === 4 ? "MIN" : "MAX", generatedAst(random, depth + 1), generatedAst(random, depth + 1));
  return binary(["+", "-", "*", "/", "%"][Math.floor(random() * 5)]!, generatedAst(random, depth + 1), generatedAst(random, depth + 1));
}

test("seeded valid AST property cases return a finite value or typed failure without mutation or throws", () => {
  const random = seededRandom(0x51afcafe);
  const inputs = new Map([["x", 3.25]]);
  const before = [...inputs.entries()];
  for (let index = 0; index < 250; index += 1) {
    const ast = generatedAst(random);
    const astBefore = JSON.stringify(ast);
    let result: ReturnType<typeof evaluate> | undefined;
    assert.doesNotThrow(() => { result = evaluate(ast, inputs); });
    assert.ok(result);
    if (result.ok) assert.equal(Number.isFinite(result.value), true);
    else assert.ok(["DIVIDE_BY_ZERO", "NON_FINITE", "INVALID_AST", "INVALID_BOUNDS", "MISSING_INPUT", "INVALID_INPUTS"].includes(result.reason));
    assert.equal(JSON.stringify(ast), astBefore);
  }
  assert.deepEqual([...inputs.entries()], before);
});
