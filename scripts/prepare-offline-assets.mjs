#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { isMainModule } from "./module-entry.mjs";

const CHI_SIM_URL =
  "https://github.com/tesseract-ocr/tessdata_fast/raw/main/chi_sim.traineddata";
const PDFIUM_RELEASE_TAG = "chromium/7897";
const PDFIUM_RELEASE_URL =
  "https://github.com/run-llama/pdfium-binaries/releases/download";

export function destinationTessdataPath(projectRoot = process.cwd()) {
  return path.join(
    projectRoot,
    "src-tauri",
    "resources",
    "tessdata",
    "chi_sim.traineddata",
  );
}

export function platformPdfiumLibraryName(platform = process.platform) {
  if (platform === "win32") {
    return "pdfium.dll";
  }
  if (platform === "darwin") {
    return "libpdfium.dylib";
  }
  return "libpdfium.so";
}

export function destinationPdfiumPath({
  projectRoot = process.cwd(),
  platform = process.platform,
} = {}) {
  return path.join(
    projectRoot,
    "src-tauri",
    "resources",
    "pdfium",
    platformPdfiumLibraryName(platform),
  );
}

export function platformPdfiumAssetStem({
  platform = process.platform,
  arch = process.arch,
} = {}) {
  if (platform === "darwin") {
    if (arch === "arm64") {
      return "pdfium-mac-arm64";
    }
    if (arch === "x64") {
      return "pdfium-mac-x64";
    }
  }

  if (platform === "win32") {
    if (arch === "arm64") {
      return "pdfium-win-arm64";
    }
    if (arch === "ia32") {
      return "pdfium-win-x86";
    }
    return "pdfium-win-x64";
  }

  if (platform === "linux") {
    if (arch === "arm64") {
      return "pdfium-linux-arm64";
    }
    if (arch === "arm") {
      return "pdfium-linux-arm";
    }
    return "pdfium-linux-x64";
  }

  throw new Error(`Unsupported platform for PDFium asset: ${platform}/${arch}`);
}

export function platformTessdataCandidates({
  env = process.env,
  platform = process.platform,
} = {}) {
  if (platform === "darwin" && env.HOME) {
    return [
      path.join(
        env.HOME,
        "Library",
        "Application Support",
        "tesseract-rs",
        "tessdata",
        "chi_sim.traineddata",
      ),
    ];
  }

  if (platform === "win32") {
    const roaming =
      env.APPDATA ||
      (env.USERPROFILE && path.join(env.USERPROFILE, "AppData", "Roaming"));
    return roaming
      ? [path.join(roaming, "tesseract-rs", "tessdata", "chi_sim.traineddata")]
      : [];
  }

  return env.HOME
    ? [
        path.join(
          env.HOME,
          ".tesseract-rs",
          "tessdata",
          "chi_sim.traineddata",
        ),
      ]
    : [];
}

function pdfiumCacheBase({ env, platform }) {
  if (env.XDG_CACHE_HOME) {
    return env.XDG_CACHE_HOME;
  }
  if (platform === "win32") {
    return (
      env.LOCALAPPDATA ||
      (env.USERPROFILE && path.join(env.USERPROFILE, "AppData", "Local"))
    );
  }
  if (platform === "darwin" && env.HOME) {
    return path.join(env.HOME, "Library", "Caches");
  }
  return env.HOME && path.join(env.HOME, ".cache");
}

function pdfiumFileCandidatesFromDir(dir, { platform = process.platform } = {}) {
  const name = platformPdfiumLibraryName(platform);
  const candidates = [path.join(dir, name)];
  if (platform === "win32") {
    candidates.push(path.join(path.dirname(dir), "bin", name));
  }
  candidates.push(path.join(dir, "bin", name), path.join(dir, "lib", name));
  return candidates;
}

export function platformPdfiumCandidates({
  env = process.env,
  platform = process.platform,
  arch = process.arch,
} = {}) {
  const candidates = [];
  if (env.RUSTITLER_PDFIUM_DIR) {
    candidates.push(
      ...pdfiumFileCandidatesFromDir(env.RUSTITLER_PDFIUM_DIR, { platform }),
    );
  }
  if (env.PDFIUM_LIB_PATH) {
    candidates.push(...pdfiumFileCandidatesFromDir(env.PDFIUM_LIB_PATH, { platform }));
  }

  const cacheBase = pdfiumCacheBase({ env, platform });
  if (cacheBase) {
    const cacheDir = path.join(
      cacheBase,
      "pdfium-rs",
      PDFIUM_RELEASE_TAG.replace("/", "_"),
      platformPdfiumAssetStem({ platform, arch }),
    );
    candidates.push(...pdfiumFileCandidatesFromDir(cacheDir, { platform }));
  }

  return candidates;
}

export function findChiSimSource({
  explicitSource,
  env = process.env,
  platform = process.platform,
  exists = fs.existsSync,
} = {}) {
  if (explicitSource) {
    return explicitSource;
  }

  const candidates = [
    env.RUSTITLER_TESSDATA &&
      path.join(env.RUSTITLER_TESSDATA, "chi_sim.traineddata"),
    env.TESSDATA_PREFIX && path.join(env.TESSDATA_PREFIX, "chi_sim.traineddata"),
    ...platformTessdataCandidates({ env, platform }),
  ].filter(Boolean);

  return candidates.find((candidate) => exists(candidate));
}

export function findPdfiumSource({
  explicitSource,
  env = process.env,
  platform = process.platform,
  arch = process.arch,
  exists = fs.existsSync,
  stat = fs.statSync,
} = {}) {
  if (explicitSource) {
    if (exists(explicitSource) && stat(explicitSource).isFile()) {
      return explicitSource;
    }
    if (exists(explicitSource) && stat(explicitSource).isDirectory()) {
      return pdfiumFileCandidatesFromDir(explicitSource, { platform }).find((candidate) =>
        exists(candidate),
      );
    }
    return explicitSource;
  }

  return platformPdfiumCandidates({ env, platform, arch }).find((candidate) =>
    exists(candidate),
  );
}

function parseArgs(argv) {
  const options = {
    download: false,
    source: undefined,
    pdfiumSource: undefined,
    projectRoot: process.cwd(),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--download") {
      options.download = true;
    } else if (arg === "--source") {
      options.source = argv[++index];
    } else if (arg === "--pdfium-source") {
      options.pdfiumSource = argv[++index];
    } else if (arg === "--project-root") {
      options.projectRoot = path.resolve(argv[++index]);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

async function downloadChiSim(destination) {
  const response = await fetch(CHI_SIM_URL);
  if (!response.ok) {
    throw new Error(`Failed to download chi_sim.traineddata: ${response.status}`);
  }
  const data = Buffer.from(await response.arrayBuffer());
  fs.writeFileSync(destination, data);
}

async function downloadPdfium(destination, { platform, arch }) {
  const asset = `${platformPdfiumAssetStem({ platform, arch })}.tgz`;
  const tag = PDFIUM_RELEASE_TAG.replace("/", "%2F");
  const url = `${PDFIUM_RELEASE_URL}/${tag}/${asset}`;
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to download PDFium ${asset}: ${response.status}`);
  }

  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "rustitler-pdfium-"));
  try {
    const archivePath = path.join(tempDir, asset);
    fs.writeFileSync(archivePath, Buffer.from(await response.arrayBuffer()));
    execFileSync("tar", ["-xzf", archivePath, "-C", tempDir]);

    const libraryName = platformPdfiumLibraryName(platform);
    const extracted = findFileByName(tempDir, libraryName);
    if (!extracted) {
      throw new Error(`Downloaded ${asset} did not contain ${libraryName}`);
    }
    fs.copyFileSync(extracted, destination);
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

function findFileByName(dir, name) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isFile() && entry.name === name) {
      return entryPath;
    }
    if (entry.isDirectory()) {
      const nested = findFileByName(entryPath, name);
      if (nested) {
        return nested;
      }
    }
  }
  return undefined;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const tessdataDestination = destinationTessdataPath(options.projectRoot);
  fs.mkdirSync(path.dirname(tessdataDestination), { recursive: true });

  const source = findChiSimSource({ explicitSource: options.source });
  if (source) {
    fs.copyFileSync(source, tessdataDestination);
    console.log(`Copied chi_sim.traineddata from ${source}`);
    console.log(`Prepared ${tessdataDestination}`);
  } else if (options.download) {
    await downloadChiSim(tessdataDestination);
    console.log(`Downloaded chi_sim.traineddata to ${tessdataDestination}`);
  } else {
    throw new Error(
      [
        "chi_sim.traineddata not found.",
        "Set RUSTITLER_TESSDATA/TESSDATA_PREFIX, pass --source <file>,",
        "or rerun with --download when network access is allowed.",
      ].join(" "),
    );
  }

  const pdfiumDestination = destinationPdfiumPath({
    projectRoot: options.projectRoot,
  });
  fs.mkdirSync(path.dirname(pdfiumDestination), { recursive: true });
  const pdfiumSource = findPdfiumSource({
    explicitSource: options.pdfiumSource,
  });

  if (pdfiumSource) {
    fs.copyFileSync(pdfiumSource, pdfiumDestination);
    console.log(`Copied ${path.basename(pdfiumDestination)} from ${pdfiumSource}`);
    console.log(`Prepared ${pdfiumDestination}`);
  } else if (options.download) {
    await downloadPdfium(pdfiumDestination, {
      platform: process.platform,
      arch: process.arch,
    });
    console.log(`Downloaded ${path.basename(pdfiumDestination)} to ${pdfiumDestination}`);
  } else {
    throw new Error(
      [
        `${platformPdfiumLibraryName()} not found.`,
        "Set RUSTITLER_PDFIUM_DIR/PDFIUM_LIB_PATH, pass --pdfium-source <file-or-dir>,",
        "or rerun with --download when network access is allowed.",
      ].join(" "),
    );
  }
}

if (isMainModule(import.meta.url, process.argv[1])) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
