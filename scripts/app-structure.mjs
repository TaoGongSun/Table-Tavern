import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const sourcePath = path.resolve(process.argv[2] ?? "src/App.tsx");
const configPath = ts.findConfigFile(process.cwd(), ts.sys.fileExists, "tsconfig.json");
if (!configPath) throw new Error("找不到 tsconfig.json；請從 repo root 執行");

const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
if (configFile.error) throw new Error(ts.flattenDiagnosticMessageText(configFile.error.messageText, "\n"));
const parsed = ts.parseJsonConfigFileContent(configFile.config, ts.sys, path.dirname(configPath));
const program = ts.createProgram(parsed.fileNames, parsed.options);
const source = program.getSourceFile(sourcePath);
if (!source) throw new Error(`TypeScript program 找不到 ${sourcePath}`);
const checker = program.getTypeChecker();
const lineOf = (position) => source.getLineAndCharacterOfPosition(position).line + 1;

const app = source.statements.find(
  (statement) => ts.isFunctionDeclaration(statement) && statement.name?.text === "App",
);
if (!app?.body) throw new Error("找不到 function App()");

// 只收 App() 直屬的 useState/useRef binding；用 symbol 比對，避免把物件 key、
// 同名區域變數或 property access 誤判成 App state。
const appBindings = new Map();
const declarations = [];
for (const statement of app.body.statements) {
  if (!ts.isVariableStatement(statement)) continue;
  for (const declaration of statement.declarationList.declarations) {
    const init = declaration.initializer;
    if (
      !init ||
      !ts.isCallExpression(init) ||
      !ts.isIdentifier(init.expression) ||
      !["useState", "useRef"].includes(init.expression.text)
    ) {
      continue;
    }
    const kind = init.expression.text === "useState" ? "state" : "ref";
    const identifiers = [];
    const collectIdentifiers = (node) => {
      if (ts.isIdentifier(node)) identifiers.push(node);
      else node.forEachChild(collectIdentifiers);
    };
    collectIdentifiers(declaration.name);
    for (const identifier of identifiers) {
      const symbol = checker.getSymbolAtLocation(identifier);
      if (symbol) appBindings.set(symbol, { name: identifier.text, kind });
    }
    declarations.push({ kind, names: identifiers.map((identifier) => identifier.text) });
  }
}

// 沿用原交叉表的「區塊」定義，但邊界直接取 AST node。
const blocks = [];
for (const statement of app.body.statements) {
  if (ts.isFunctionDeclaration(statement) && statement.name) {
    blocks.push({ name: statement.name.text, kind: "fn", node: statement });
  }
  if (ts.isVariableStatement(statement)) {
    for (const declaration of statement.declarationList.declarations) {
      const init = declaration.initializer;
      const name = ts.isIdentifier(declaration.name)
        ? declaration.name.text
        : declaration.name.getText(source);
      if (init && (ts.isArrowFunction(init) || ts.isFunctionExpression(init))) {
        blocks.push({ name, kind: "fn", node: declaration });
      } else if (
        init &&
        ts.isCallExpression(init) &&
        ts.isIdentifier(init.expression) &&
        ["useMemo", "useCallback"].includes(init.expression.text)
      ) {
        blocks.push({ name, kind: init.expression.text === "useMemo" ? "memo" : "fn", node: declaration });
      }
    }
  }
  if (
    ts.isExpressionStatement(statement) &&
    ts.isCallExpression(statement.expression) &&
    ts.isIdentifier(statement.expression.expression) &&
    statement.expression.expression.text === "useEffect"
  ) {
    blocks.push({
      name: `useEffect@${lineOf(statement.getStart(source))}`,
      kind: "effect",
      node: statement,
    });
  }
  if (ts.isReturnStatement(statement)) {
    blocks.push({ name: "__JSX__", kind: "jsx", node: statement });
  }
}

const byBinding = new Map([...appBindings.values()].map(({ name }) => [name, new Set()]));
for (const block of blocks) {
  block.touched = new Set();
  const visit = (node) => {
    if (ts.isIdentifier(node)) {
      const binding = appBindings.get(checker.getSymbolAtLocation(node));
      if (binding) block.touched.add(binding.name);
    }
    node.forEachChild(visit);
  };
  visit(block.node);
  for (const name of block.touched) byBinding.get(name).add(block.name);
}

let invokeCalls = 0;
const invokeCommands = new Set();
const visitInvokes = (node) => {
  if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "invoke") {
    invokeCalls += 1;
    const command = node.arguments[0];
    if (command && ts.isStringLiteralLike(command)) invokeCommands.add(command.text);
  }
  node.forEachChild(visitInvokes);
};
visitInvokes(source);

const appStart = lineOf(app.getStart(source));
const appEnd = lineOf(app.end);
const stateCount = declarations.filter(({ kind }) => kind === "state").length;
const refCount = declarations.filter(({ kind }) => kind === "ref").length;
console.log(
  `# App() = ${appStart}-${appEnd}（${appEnd - appStart + 1} 行）  state ${stateCount}／ref ${refCount}／區塊 ${blocks.length}`,
);
console.log(`# 全檔 invoke ${invokeCalls} 次／${invokeCommands.size} 個不同字面指令`);
console.log("\n## 區塊");
for (const block of blocks.sort((a, b) => a.node.getStart(source) - b.node.getStart(source))) {
  const start = lineOf(block.node.getStart(source));
  const end = lineOf(block.node.end);
  const touched = [...block.touched].sort();
  console.log(
    `${String(end - start + 1).padStart(5)}行 ${String(start).padStart(5)}-${String(end).padEnd(5)} ` +
      `[${block.kind.padEnd(6)}] ${block.name.padEnd(32)} x${String(touched.length).padEnd(3)} ${touched.join(" ")}`,
  );
}

console.log("\n## state/ref binding 被幾個區塊觸及");
for (const [name, touchedBy] of [...byBinding].sort((a, b) => b[1].size - a[1].size || a[0].localeCompare(b[0]))) {
  console.log(`${String(touchedBy.size).padStart(3)} x ${name.padEnd(28)} ${[...touchedBy].sort().join(" ")}`);
}
