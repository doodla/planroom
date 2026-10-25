import { readFileSync } from 'node:fs';
import { parse } from 'yaml';
import { loadConfig } from './config.js';

interface FlagFile {
  flags: Record<string, { enabled: boolean; description?: string }>;
}

let cache: FlagFile | undefined;

function load(): FlagFile {
  if (!cache) {
    const text = readFileSync(loadConfig().flagsPath, 'utf8');
    cache = parse(text) as FlagFile;
  }
  return cache;
}

/** Reports whether a flag in flags.yaml is on. Unknown flags are off. */
export function isEnabled(name: string): boolean {
  return load().flags[name]?.enabled === true;
}

/** For tests: forget the cached file. */
export function resetFlags(): void {
  cache = undefined;
}
