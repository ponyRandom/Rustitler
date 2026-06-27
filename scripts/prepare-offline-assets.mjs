#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { isMainModule } from "./module-entry.mjs";

const CHI_SIM_URL =
  "https://github.com/tesseract-ocr/tessdata_fast/raw/main/chi_sim.traineddata";

export function destinationTessdataPath(projectRoot = process.cwd()) {
  return path.join(
    projectRoot,
    "src-tauri",
    "resources",
    "tessdata",
    "chi_sim.traineddata",
  );
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

function parseArgs(argv) {
  const options = {
    download: false,
    source: undefined,
    projectRoot: process.cwd(),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--download") {
      options.download = true;
    } else if (arg === "--source") {
      options.source = argv[++index];
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

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const destination = destinationTessdataPath(options.projectRoot);
  fs.mkdirSync(path.dirname(destination), { recursive: true });

  const source = findChiSimSource({ explicitSource: options.source });
  if (source) {
    fs.copyFileSync(source, destination);
    console.log(`Copied chi_sim.traineddata from ${source}`);
    console.log(`Prepared ${destination}`);
    return;
  }

  if (options.download) {
    await downloadChiSim(destination);
    console.log(`Downloaded chi_sim.traineddata to ${destination}`);
    return;
  }

  throw new Error(
    [
      "chi_sim.traineddata not found.",
      "Set RUSTITLER_TESSDATA/TESSDATA_PREFIX, pass --source <file>,",
      "or rerun with --download when network access is allowed.",
    ].join(" "),
  );
}

if (isMainModule(import.meta.url, process.argv[1])) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
