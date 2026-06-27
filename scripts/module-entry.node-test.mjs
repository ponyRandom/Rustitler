import assert from "node:assert/strict";
import test from "node:test";
import { isMainModule } from "./module-entry.mjs";

test("isMainModule matches Windows argv paths", () => {
  assert.equal(
    isMainModule(
      "file:///D:/a/Rustitler/Rustitler/scripts/package-size-report.mjs",
      "D:\\a\\Rustitler\\Rustitler\\scripts\\package-size-report.mjs",
    ),
    true,
  );
});

test("isMainModule rejects different entry paths", () => {
  assert.equal(
    isMainModule(
      "file:///repo/scripts/package-size-report.mjs",
      "/repo/scripts/prepare-offline-assets.mjs",
    ),
    false,
  );
});
