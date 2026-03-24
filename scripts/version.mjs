#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const versionFiles = {
  packageJson: resolve(rootDir, "package.json"),
  cargoToml: resolve(rootDir, "src-tauri", "Cargo.toml"),
  tauriConfig: resolve(rootDir, "src-tauri", "tauri.conf.json"),
  benchmarkConfig: resolve(rootDir, "src-tauri", "tauri.benchmark.conf.json"),
};

const SEMVER_PATTERN = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

function normalizeVersion(value) {
  const normalized = value.trim().replace(/^v/, "");
  if (!SEMVER_PATTERN.test(normalized)) {
    throw new Error(
      `Invalid version "${value}". Expected SemVer like 0.2.0 or 0.2.0-beta.1.`,
    );
  }
  return normalized;
}

async function updateJsonVersion(filePath, version) {
  const raw = await readFile(filePath, "utf8");
  const parsed = JSON.parse(raw);
  parsed.version = version;
  await writeFile(filePath, `${JSON.stringify(parsed, null, 2)}\n`, "utf8");
}

async function updateCargoVersion(filePath, version) {
  const raw = await readFile(filePath, "utf8");
  const updated = raw.replace(
    /^version = ".*"$/m,
    `version = "${version}"`,
  );

  if (updated === raw) {
    throw new Error(`Could not find package version in ${filePath}.`);
  }

  await writeFile(filePath, updated, "utf8");
}

async function readVersions() {
  const [packageJsonRaw, cargoTomlRaw, tauriConfigRaw, benchmarkConfigRaw] =
    await Promise.all([
      readFile(versionFiles.packageJson, "utf8"),
      readFile(versionFiles.cargoToml, "utf8"),
      readFile(versionFiles.tauriConfig, "utf8"),
      readFile(versionFiles.benchmarkConfig, "utf8"),
    ]);

  const cargoVersionMatch = cargoTomlRaw.match(/^version = "(.*)"$/m);
  if (!cargoVersionMatch) {
    throw new Error("Could not read package version from src-tauri/Cargo.toml.");
  }

  return {
    packageJson: JSON.parse(packageJsonRaw).version,
    cargoToml: cargoVersionMatch[1],
    tauriConfig: JSON.parse(tauriConfigRaw).version,
    benchmarkConfig: JSON.parse(benchmarkConfigRaw).version,
  };
}

async function setVersion(rawVersion) {
  const version = normalizeVersion(rawVersion);

  await Promise.all([
    updateJsonVersion(versionFiles.packageJson, version),
    updateCargoVersion(versionFiles.cargoToml, version),
    updateJsonVersion(versionFiles.tauriConfig, version),
    updateJsonVersion(versionFiles.benchmarkConfig, version),
  ]);

  console.log(`Updated Orchestrix version to ${version}.`);
  console.log("- package.json");
  console.log("- src-tauri/Cargo.toml");
  console.log("- src-tauri/tauri.conf.json");
  console.log("- src-tauri/tauri.benchmark.conf.json");
}

async function assertTag(rawTag) {
  const expectedVersion = normalizeVersion(rawTag);
  const versions = await readVersions();

  for (const [name, version] of Object.entries(versions)) {
    if (version !== expectedVersion) {
      throw new Error(
        `${name} is ${version}, but the release tag expects ${expectedVersion}.`,
      );
    }
  }

  console.log(`Release tag ${rawTag} matches every tracked version file.`);
}

async function main() {
  const [command, value] = process.argv.slice(2);

  if (command === "set") {
    if (!value) {
      throw new Error("Usage: bun run version:set <version>");
    }
    await setVersion(value);
    return;
  }

  if (command === "assert-tag") {
    if (!value) {
      throw new Error("Usage: bun run release:assert-tag <tag>");
    }
    await assertTag(value);
    return;
  }

  throw new Error(
    "Usage: bun run scripts/version.mjs <set|assert-tag> <value>",
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
