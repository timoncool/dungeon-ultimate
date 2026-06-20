import { spawn } from "node:child_process";
import {
  closeSync,
  mkdirSync,
  openSync,
  readFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { z } from "zod";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const actionSchema = z.object({
  action: z.enum(["start", "open-model-folder"]),
});

let cachedEnvServer: Record<string, string> | null = null;

function loadEnvServer() {
  if (cachedEnvServer) {
    return cachedEnvServer;
  }

  try {
    cachedEnvServer = Object.fromEntries(
      readFileSync(".env.server", "utf8")
        .split(/\r?\n/)
        .map((line) => line.trim())
        .filter((line) => line && !line.startsWith("#"))
        .map((line) => {
          const separator = line.indexOf("=");
          if (separator < 0) {
            return ["", ""] as const;
          }
          return [line.slice(0, separator).trim(), line.slice(separator + 1).trim()] as const;
        })
        .filter(([key]) => key),
    );
  } catch {
    cachedEnvServer = {};
  }

  return cachedEnvServer;
}

function envValue(key: string, fallback = "") {
  return process.env[key] || loadEnvServer()[key] || fallback;
}

function expandHome(value: string) {
  if (value === "~") return os.homedir();
  if (value.startsWith("~/") || value.startsWith("~\\")) {
    return path.join(os.homedir(), value.slice(2));
  }
  return value;
}

async function waitForHealth(seconds: number) {
  const workerUrl = envValue("FLUX_WORKER_URL", "http://127.0.0.1:7869").replace(/\/$/, "");
  const deadline = Date.now() + seconds * 1000;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${workerUrl}/health`, { cache: "no-store" });
      if (response.ok) {
        return await response.json();
      }
    } catch {
      // Keep polling until the short startup window closes.
    }
    await new Promise((resolve) => setTimeout(resolve, 750));
  }

  return null;
}

function modelFolder() {
  const mfluxDir = expandHome(
    envValue("MFLUX_DIR") ||
      envValue("ULTRA_FAST_MFLUX_HS_DIR") ||
      path.join(os.homedir(), ".cache", "ultra-fast-image-gen", "mflux"),
  );

  return path.dirname(mfluxDir);
}

function openFolder(folder: string) {
  mkdirSync(folder, { recursive: true });

  const command =
    process.platform === "darwin"
      ? "open"
      : process.platform === "win32"
        ? "explorer"
        : "xdg-open";
  const child = spawn(command, [folder], {
    detached: true,
    stdio: "ignore",
  });
  child.unref();
}

function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}

async function startWorker() {
  const existing = await waitForHealth(1);
  if (existing) {
    return Response.json({
      ok: true,
      status: "running",
      message: "Рабочий для изображений уже работает.",
      health: existing,
    });
  }

  const logDir = "logs";
  mkdirSync(logDir, { recursive: true });
  const logPath = path.join(logDir, "image-worker-ui.log");
  const logFd = openSync(logPath, "a");

  try {
    const child = spawn(npmCommand(), ["run", "image:server"], {
      env: { ...process.env, ...loadEnvServer() },
      detached: true,
      stdio: ["ignore", logFd, logFd],
    });
    child.on("error", () => {
      // The API returns the log path below; avoid an unhandled process error if npm
      // disappears after the spawn call.
    });
    child.unref();
  } catch (error) {
    return Response.json(
      {
        ok: false,
        error: error instanceof Error ? error.message : String(error),
        logPath,
      },
      { status: 500 },
    );
  } finally {
    closeSync(logFd);
  }

  const health = await waitForHealth(8);
  return Response.json(
    {
      ok: Boolean(health),
      status: health ? "running" : "starting",
      message: health
        ? "Image worker started."
        : "Рабочий для изображений запускается. Если он не появится вскоре, проверьте журнал.",
      health,
      logPath,
    },
    { status: health ? 200 : 202 },
  );
}

export async function POST(request: Request) {
  const body = actionSchema.parse(await request.json().catch(() => ({})));

  if (body.action === "open-model-folder") {
    const folder = modelFolder();
    openFolder(folder);
    return Response.json({ ok: true, path: folder });
  }

  return startWorker();
}
