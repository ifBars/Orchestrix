import readline from "node:readline";
import { env, pipeline } from "@xenova/transformers";

const pipelineCache = new Map();

function cacheKey(request) {
  return JSON.stringify({
    model: request.model,
    device: request.device,
    backend: request.backend ?? null,
    cacheDir: request.cacheDir ?? null,
  });
}

async function getExtractor(request) {
  const key = cacheKey(request);
  const cached = pipelineCache.get(key);
  if (cached) {
    return cached;
  }

  if (request.cacheDir && request.cacheDir.trim().length > 0) {
    env.cacheDir = request.cacheDir;
  }

  const extractor = await pipeline("feature-extraction", request.model, {
    device: request.device,
  });
  pipelineCache.set(key, extractor);
  return extractor;
}

function toVectors(output) {
  if (!output || !output.data || !output.dims) {
    throw new Error("transformers output is missing tensor data");
  }

  const dims = output.dims;
  const data = output.data;
  if (!Array.isArray(dims) || dims.length < 2) {
    throw new Error("unexpected tensor shape returned by transformers pipeline");
  }

  const rows = Number(dims[0]);
  const cols = Number(dims[dims.length - 1]);
  if (!Number.isFinite(rows) || !Number.isFinite(cols) || rows <= 0 || cols <= 0) {
    throw new Error("invalid tensor shape returned by transformers pipeline");
  }

  const vectors = [];
  for (let row = 0; row < rows; row += 1) {
    const start = row * cols;
    const end = start + cols;
    vectors.push(Array.from(data.slice(start, end), (value) => Number(value)));
  }
  return vectors;
}

async function handleRequest(request) {
  if (!request || typeof request !== "object") {
    throw new Error("request payload must be an object");
  }
  if (!request.model || !request.device) {
    throw new Error("request must include model and device");
  }

  const extractor = await getExtractor(request);
  if (request.action === "dims") {
    const output = await extractor(["dimensions probe"], {
      pooling: "mean",
      normalize: false,
    });
    const vectors = toVectors(output);
    return {
      dims: vectors[0]?.length ?? null,
    };
  }

  if (request.action === "embed") {
    if (!Array.isArray(request.texts)) {
      throw new Error("embed action requires texts array");
    }

    const output = await extractor(request.texts, {
      pooling: "mean",
      normalize: false,
    });
    const vectors = toVectors(output);
    return {
      vectors,
      dims: vectors[0]?.length ?? null,
    };
  }

  throw new Error(`unsupported action: ${String(request.action)}`);
}

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Number.POSITIVE_INFINITY,
});

for await (const line of rl) {
  const trimmed = line.trim();
  if (!trimmed) {
    continue;
  }

  let id = null;
  try {
    const request = JSON.parse(trimmed);
    id = request.id ?? null;
    const result = await handleRequest(request);
    process.stdout.write(
      `${JSON.stringify({ id, ok: true, result })}\n`,
    );
  } catch (error) {
    process.stdout.write(
      `${JSON.stringify({
        id,
        ok: false,
        error: error instanceof Error ? error.message : String(error),
      })}\n`,
    );
  }
}
