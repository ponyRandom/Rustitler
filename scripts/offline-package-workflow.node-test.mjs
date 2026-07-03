import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

const workflow = fs.readFileSync(".github/workflows/offline-package.yml", "utf8");

test("Windows smoke step waits for the packaged GUI process and report file", () => {
  assert.match(workflow, /Start-Process[\s\S]*-Wait[\s\S]*-PassThru/);
  assert.match(workflow, /for \(\$attempt = 0; \$attempt -lt 30; \$attempt\+\+\)/);
  assert.match(workflow, /Test-Path \$smokeReport/);
  assert.match(workflow, /Start-Sleep -Seconds 1/);
});

test("release job publishes only short installer asset names", () => {
  assert.match(workflow, /node scripts\/collect-release-assets\.mjs/);
  assert.match(workflow, /--product-name Rustitler/);
  assert.match(workflow, /--tag "\$GITHUB_REF_NAME"/);
  assert.match(workflow, /name: rustitler-macos-offline-package-\$\{\{ matrix\.variant \}\}/);
  assert.match(workflow, /name: rustitler-windows-offline-package-\$\{\{ matrix\.variant \}\}/);
  assert.doesNotMatch(workflow, /-name "\*\.msi"/);
  assert.doesNotMatch(workflow, /-name "\*\.md"/);
  assert.doesNotMatch(workflow, /-name "\*\.json"/);
  assert.doesNotMatch(workflow, /release-assets\/\$\{artifact\}-\$\{name\}/);
});

test("package jobs build both LibreOffice variants", () => {
  assert.match(workflow, /variant: without-libreoffice/);
  assert.match(workflow, /variant: with-libreoffice/);
  assert.match(workflow, /Install LibreOffice runtime/);
  assert.match(workflow, /Prepare bundled LibreOffice runtime/);
  assert.match(workflow, /pdfiumPresent/);
  assert.match(workflow, /sofficePresent/);
  assert.match(workflow, /Release assets include four installers/);
});
