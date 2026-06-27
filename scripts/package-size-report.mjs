#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

export function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1024;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}

export function measurePathBytes(targetPath) {
  if (!fs.existsSync(targetPath)) {
    return 0;
  }

  const stat = fs.statSync(targetPath);
  if (stat.isFile()) {
    return stat.size;
  }
  if (!stat.isDirectory()) {
    return 0;
  }

  return fs
    .readdirSync(targetPath)
    .map((entry) => measurePathBytes(path.join(targetPath, entry)))
    .reduce((total, size) => total + size, 0);
}

export function buildSizeReport({
  projectRoot = process.cwd(),
  bundleDir = path.join(projectRoot, "src-tauri", "target", "release", "bundle"),
  platform = process.platform,
} = {}) {
  const tessdataDir = path.join(
    projectRoot,
    "src-tauri",
    "resources",
    "tessdata",
  );
  const libreofficeDir = path.join(
    projectRoot,
    "src-tauri",
    "resources",
    "libreoffice",
  );
  const rows = [
    ["Bundle total", measurePathBytes(bundleDir)],
    ["Tessdata resources", measurePathBytes(tessdataDir)],
    ["LibreOffice resources", measurePathBytes(libreofficeDir)],
  ];

  const artifactRows = listImmediateChildren(bundleDir).map((entry) => [
    `Artifact: ${path.relative(projectRoot, entry)}`,
    measurePathBytes(entry),
  ]);

  const markdown = [
    "# Offline Package Size Report",
    "",
    `Platform: ${platform}`,
    "",
    "| Component | Size | Bytes |",
    "| --- | ---: | ---: |",
    ...[...rows, ...artifactRows].map(
      ([label, bytes]) => `| ${label} | ${formatBytes(bytes)} | ${bytes} |`,
    ),
    "",
  ].join("\n");

  return { markdown, rows, artifactRows };
}

function listImmediateChildren(dir) {
  if (!fs.existsSync(dir) || !fs.statSync(dir).isDirectory()) {
    return [];
  }
  return fs.readdirSync(dir).map((entry) => path.join(dir, entry));
}

function parseArgs(argv) {
  const options = {
    projectRoot: process.cwd(),
    bundleDir: undefined,
    platform: process.platform,
    output: undefined,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--project-root") {
      options.projectRoot = path.resolve(argv[++index]);
    } else if (arg === "--bundle-dir") {
      options.bundleDir = path.resolve(argv[++index]);
    } else if (arg === "--platform") {
      options.platform = argv[++index];
    } else if (arg === "--output") {
      options.output = path.resolve(argv[++index]);
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function appendStepSummary(markdown) {
  if (!process.env.GITHUB_STEP_SUMMARY) {
    return;
  }
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${markdown}\n`);
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const { markdown } = buildSizeReport({
    projectRoot: options.projectRoot,
    bundleDir:
      options.bundleDir ||
      path.join(options.projectRoot, "src-tauri", "target", "release", "bundle"),
    platform: options.platform,
  });

  if (options.output) {
    fs.mkdirSync(path.dirname(options.output), { recursive: true });
    fs.writeFileSync(options.output, markdown);
  }
  appendStepSummary(markdown);
  console.log(markdown);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}
