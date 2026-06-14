#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { homedir } from "node:os";

const args = process.argv.slice(2);

function readFlag(name) {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith("--")) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

function hasFlag(name) {
  return args.includes(name);
}

function usage() {
  console.log(`Usage:
  npm run image:codex -- --out public/sidebar-icons/chats.png --prompt "a dungeon scroll icon"
  npm run image:codex -- --out public/sidebar-icons/chats.png --prompt-file prompts/chats.txt

Options:
  --out <path>          Required output PNG path.
  --prompt <text>       Prompt text.
  --prompt-file <path>  Prompt file, used when --prompt is omitted.
  --keep-message        Keep the temporary final-message text file.

Environment:
  CODEX_BIN             Override Codex executable path.
  CODEX_HOME            Override Codex home. Defaults to ~/.codex.
`);
}

if (hasFlag("--help") || hasFlag("-h")) {
  usage();
  process.exit(0);
}

const outPath = readFlag("--out");
const promptArg = readFlag("--prompt");
const promptFile = readFlag("--prompt-file");

if (!outPath) {
  usage();
  throw new Error("--out is required");
}

const prompt = promptArg ?? (promptFile ? readFileSync(promptFile, "utf8") : "");
if (!prompt.trim()) {
  usage();
  throw new Error("--prompt or --prompt-file is required");
}

const codexHome = process.env.CODEX_HOME || join(homedir(), ".codex");
const standaloneCodex = join(homedir(), ".local", "bin", "codex");
const codexBin = process.env.CODEX_BIN || (existsSync(standaloneCodex) ? standaloneCodex : "codex");
const markerMs = Date.now();
const messagePath = join(
  codexHome,
  ".tmp",
  `codex-imagegen-${process.pid}-${Math.random().toString(16).slice(2)}.txt`,
);

const instruction = `Use the built-in image generation tool exactly once.
Generate one raster PNG image for this prompt:

${prompt.trim()}

Do not create the image with shell scripts, SVG, canvas, local image servers, or existing files.
After the image generation call is complete, reply only with IMAGE_DONE.`;

const result = spawnSync(
  codexBin,
  [
    "exec",
    "--dangerously-bypass-approvals-and-sandbox",
    "--enable",
    "image_generation",
    "--output-last-message",
    messagePath,
    instruction,
  ],
  {
    cwd: process.cwd(),
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  },
);

const combinedOutput = `${result.stdout || ""}\n${result.stderr || ""}`;
if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  throw new Error(`codex exec failed with status ${result.status}\n${combinedOutput}`);
}

const sessionId = combinedOutput.match(/session id:\s*([0-9a-f-]+)/i)?.[1];
const rolloutPath = findRolloutPath(codexHome, sessionId, markerMs);
if (!rolloutPath) {
  throw new Error(`Could not find Codex rollout for image generation${sessionId ? ` (${sessionId})` : ""}`);
}

const image = extractLastImage(rolloutPath);
if (!image?.base64) {
  throw new Error(`No image_generation_call.result found in ${rolloutPath}`);
}

const buffer = Buffer.from(image.base64, "base64");
if (!isPng(buffer)) {
  throw new Error(`Generated image payload was not a PNG (${rolloutPath})`);
}

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, buffer);

const metadataPath = outPath.replace(/\.png$/i, ".json");
writeFileSync(
  metadataPath,
  `${JSON.stringify(
    {
      source: "codex exec image_generation",
      codexBin,
      rolloutPath,
      imageId: image.id,
      revisedPrompt: image.revisedPrompt,
      prompt: prompt.trim(),
    },
    null,
    2,
  )}\n`,
);

if (!hasFlag("--keep-message") && existsSync(messagePath)) {
  try {
    rmSync(messagePath, { force: true });
  } catch {
    // Best effort only; the generated image is already persisted.
  }
}

console.log(`Saved ${outPath}`);
console.log(`Metadata ${metadataPath}`);
console.log(`Rollout ${rolloutPath}`);

function findRolloutPath(home, id, sinceMs) {
  const sessionsDir = join(home, "sessions");
  const files = [];
  walk(sessionsDir, files);

  if (id) {
    const exact = files.find((file) => file.endsWith(`${id}.jsonl`));
    if (exact) return exact;
  }

  return files
    .filter((file) => {
      try {
        return statSync(file).mtimeMs >= sinceMs - 1000 && readFileSync(file, "utf8").includes("image_generation_call");
      } catch {
        return false;
      }
    })
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
}

function walk(dir, files) {
  if (!existsSync(dir)) return;
  for (const entry of safeReadDir(dir)) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(path, files);
    } else if (entry.isFile() && entry.name.endsWith(".jsonl")) {
      files.push(path);
    }
  }
}

function safeReadDir(dir) {
  try {
    return readdirSync(dir, { withFileTypes: true });
  } catch {
    return [];
  }
}

function extractLastImage(path) {
  let lastImage;
  for (const line of readFileSync(path, "utf8").split(/\n/)) {
    if (!line.trim()) continue;
    const record = JSON.parse(line);
    const payload = record.payload;
    if (record.type !== "response_item" || payload?.type !== "image_generation_call") continue;
    if (!payload.result) continue;
    lastImage = {
      base64: payload.result,
      id: payload.id,
      revisedPrompt: payload.revised_prompt,
    };
  }
  return lastImage;
}

function isPng(buffer) {
  return (
    buffer.length > 8 &&
    buffer[0] === 0x89 &&
    buffer[1] === 0x50 &&
    buffer[2] === 0x4e &&
    buffer[3] === 0x47 &&
    buffer[4] === 0x0d &&
    buffer[5] === 0x0a &&
    buffer[6] === 0x1a &&
    buffer[7] === 0x0a
  );
}
