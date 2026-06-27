import { pathToFileURL } from "node:url";

export function isMainModule(importMetaUrl, argvPath) {
  return Boolean(argvPath) && importMetaUrl === toFileUrl(argvPath);
}

function toFileUrl(filePath) {
  if (/^[A-Za-z]:[\\/]/.test(filePath)) {
    return pathToFileURL(`/${filePath.replaceAll("\\", "/")}`).href;
  }
  return pathToFileURL(filePath).href;
}
