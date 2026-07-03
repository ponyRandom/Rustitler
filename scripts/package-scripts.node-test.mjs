import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const packageJson = JSON.parse(fs.readFileSync("package.json", "utf8"));
const tauriConfig = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));

test("package exposes the tauri script expected by tauri-action", () => {
  assert.equal(packageJson.scripts.tauri, "tauri");
});

test("desktop dev script enables document extraction dependencies", () => {
  assert.equal(
    packageJson.scripts["tauri:dev:offline"],
    "npm run prepare:offline-assets && tauri dev --features offline-bundle",
  );
});

test("desktop build script enables document extraction dependencies", () => {
  assert.equal(
    packageJson.scripts["tauri:build:offline"],
    "npm run prepare:offline-assets && tauri build --features offline-bundle",
  );
});

test("offline bundle includes the PDFium runtime resource", () => {
  assert.equal(tauriConfig.bundle.resources["resources/pdfium"], "pdfium");
});
