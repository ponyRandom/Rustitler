import { pathToFileURL } from "node:url";

export function isMainModule(importMetaUrl, argvPath) {
  return Boolean(argvPath) && importMetaUrl === toFileUrl(argvPath);
}

function toFileUrl(filePath) {
  if (/^[A-Za-z]:[\\/]/.test(filePath)) {
    const [drive, ...segments] = filePath.replaceAll("\\", "/").split("/");
    return `file:///${drive}/${segments.map(encodeURIComponent).join("/")}`;
  }
  return pathToFileURL(filePath).href;
}
