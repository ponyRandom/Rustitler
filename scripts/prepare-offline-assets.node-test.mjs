import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import path from "node:path";
import {
  destinationPdfiumPath,
  destinationTessdataPath,
  findChiSimSource,
  findPdfiumSource,
  platformPdfiumAssetStem,
  platformPdfiumCandidates,
  platformPdfiumLibraryName,
  platformTessdataCandidates,
} from "./prepare-offline-assets.mjs";

test("destinationTessdataPath points at Tauri tessdata resources", () => {
  const root = path.resolve("/repo");

  assert.equal(
    destinationTessdataPath(root),
    path.join(root, "src-tauri", "resources", "tessdata", "chi_sim.traineddata"),
  );
});

test("findChiSimSource prefers explicit source then env prefixes then platform defaults", () => {
  const envPrefixSource = path.join("/env-prefix", "chi_sim.traineddata");
  const homeSource = path.join(
    "/home",
    "Library",
    "Application Support",
    "tesseract-rs",
    "tessdata",
    "chi_sim.traineddata",
  );
  const existing = new Set([
    envPrefixSource,
    homeSource,
  ]);

  assert.equal(
    findChiSimSource({
      explicitSource: "/manual/chi_sim.traineddata",
      env: { TESSDATA_PREFIX: "/env-prefix", HOME: "/home" },
      platform: "darwin",
      exists: (candidate) => existing.has(candidate),
    }),
    "/manual/chi_sim.traineddata",
  );
  assert.equal(
    findChiSimSource({
      env: { TESSDATA_PREFIX: "/env-prefix", HOME: "/home" },
      platform: "darwin",
      exists: (candidate) => existing.has(candidate),
    }),
    envPrefixSource,
  );
});

test("platformTessdataCandidates covers Windows APPDATA fallback", () => {
  assert.deepEqual(
    platformTessdataCandidates({
      env: { APPDATA: "C:\\Users\\runner\\AppData\\Roaming" },
      platform: "win32",
    }),
    [
      path.join(
        "C:\\Users\\runner\\AppData\\Roaming",
        "tesseract-rs",
        "tessdata",
        "chi_sim.traineddata",
      ),
    ],
  );
});

test("destinationPdfiumPath points at Tauri pdfium resources", () => {
  const root = path.resolve("/repo");

  assert.equal(
    destinationPdfiumPath({ projectRoot: root, platform: "win32" }),
    path.join(root, "src-tauri", "resources", "pdfium", "pdfium.dll"),
  );
  assert.equal(
    destinationPdfiumPath({ projectRoot: root, platform: "darwin" }),
    path.join(root, "src-tauri", "resources", "pdfium", "libpdfium.dylib"),
  );
});

test("platformPdfiumAssetStem maps Windows and macOS release artifacts", () => {
  assert.equal(
    platformPdfiumAssetStem({ platform: "win32", arch: "x64" }),
    "pdfium-win-x64",
  );
  assert.equal(
    platformPdfiumAssetStem({ platform: "darwin", arch: "arm64" }),
    "pdfium-mac-arm64",
  );
  assert.equal(platformPdfiumLibraryName("win32"), "pdfium.dll");
});

test("findPdfiumSource checks explicit directories and Windows cache fallback", () => {
  const explicit = path.join("C:\\PDFium", "bin", "pdfium.dll");
  const cache = path.join(
    "C:\\Users\\runner\\AppData\\Local",
    "pdfium-rs",
    "chromium_7897",
    "pdfium-win-x64",
    "bin",
    "pdfium.dll",
  );
  const existing = new Set(["C:\\PDFium", explicit, cache]);

  assert.equal(
    findPdfiumSource({
      explicitSource: "C:\\PDFium",
      platform: "win32",
      exists: (candidate) => existing.has(candidate),
      stat: () => ({ isFile: () => false, isDirectory: () => true }),
    }),
    explicit,
  );
  assert.equal(
    platformPdfiumCandidates({
      env: { LOCALAPPDATA: "C:\\Users\\runner\\AppData\\Local" },
      platform: "win32",
      arch: "x64",
    }).at(-2),
    cache,
  );
  assert.equal(
    findPdfiumSource({
      env: { LOCALAPPDATA: "C:\\Users\\runner\\AppData\\Local" },
      platform: "win32",
      arch: "x64",
      exists: (candidate) => existing.has(candidate),
    }),
    cache,
  );
});

test("offline asset preparation handles the PDFium runtime library", () => {
  const script = fs.readFileSync("scripts/prepare-offline-assets.mjs", "utf8");

  assert.match(script, /PDFIUM_RELEASE_TAG/);
  assert.match(script, /resources["']?,\s*["']pdfium/);
  assert.match(script, /pdfium-win-x64/);
  assert.match(script, /pdfium\.dll/);
});
