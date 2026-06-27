import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildSizeReport,
  formatBytes,
  measurePathBytes,
} from "./package-size-report.mjs";

test("formatBytes uses stable binary units", () => {
  assert.equal(formatBytes(0), "0 B");
  assert.equal(formatBytes(512), "512 B");
  assert.equal(formatBytes(1536), "1.5 KiB");
  assert.equal(formatBytes(5 * 1024 * 1024), "5.0 MiB");
});

test("measurePathBytes sums files recursively", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "rustitler-size-"));
  fs.mkdirSync(path.join(dir, "nested"));
  fs.writeFileSync(path.join(dir, "a.bin"), Buffer.alloc(3));
  fs.writeFileSync(path.join(dir, "nested", "b.bin"), Buffer.alloc(7));

  assert.equal(measurePathBytes(dir), 10);
});

test("buildSizeReport includes bundle and resource totals", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "rustitler-report-"));
  const bundleDir = path.join(root, "bundle");
  const tessdataDir = path.join(root, "src-tauri", "resources", "tessdata");
  const libreofficeDir = path.join(root, "src-tauri", "resources", "libreoffice");
  fs.mkdirSync(bundleDir, { recursive: true });
  fs.mkdirSync(tessdataDir, { recursive: true });
  fs.mkdirSync(libreofficeDir, { recursive: true });
  fs.writeFileSync(path.join(bundleDir, "Rustitler.app"), Buffer.alloc(11));
  fs.writeFileSync(path.join(tessdataDir, "chi_sim.traineddata"), Buffer.alloc(13));
  fs.writeFileSync(path.join(libreofficeDir, "soffice"), Buffer.alloc(17));

  const report = buildSizeReport({
    projectRoot: root,
    bundleDir,
    platform: "macos",
  });

  assert.match(report.markdown, /# Offline Package Size Report/);
  assert.match(report.markdown, /\| Bundle total \| 11 B \|/);
  assert.match(report.markdown, /\| Tessdata resources \| 13 B \|/);
  assert.match(report.markdown, /\| LibreOffice resources \| 17 B \|/);
});
