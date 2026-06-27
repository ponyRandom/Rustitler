import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import {
  destinationTessdataPath,
  findChiSimSource,
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
