import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, versionDefinitions, versions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const siteRoot = path.resolve(scriptDirectory, '..');
const contentRoot = path.join(siteRoot, 'content');
const generatedRoot = path.join(siteRoot, '.generated-docs');
const locales = ['zh', 'en'];
const allowedExtensions = new Set(['.json', '.md', '.mdx']);

if (versions[0] !== defaultVersion) {
  throw new Error('The default documentation version must be listed first.');
}

const localeFiles = new Map();
for (const locale of locales) {
  const files = (await collectFiles(path.join(contentRoot, locale)))
    .map((file) => path.relative(path.join(contentRoot, locale), file))
    .filter((file) => allowedExtensions.has(path.extname(file)))
    .sort();
  localeFiles.set(locale, files);
}

const [chineseFiles, englishFiles] = locales.map((locale) =>
  localeFiles.get(locale),
);
assertSameList(chineseFiles, englishFiles, 'Chinese and English source paths');

const pageFiles = chineseFiles.filter((file) => /\.(md|mdx)$/.test(file));
if (pageFiles.length < 14) {
  throw new Error(
    `Expected comprehensive documentation, found ${pageFiles.length} pages.`,
  );
}

for (const locale of locales) {
  for (const relativePath of localeFiles.get(locale)) {
    const sourcePath = path.join(contentRoot, locale, relativePath);
    const source = await readFile(sourcePath, 'utf8');
    if (/[\u2013\u2014]/u.test(source)) {
      throw new Error(`Forbidden en/em dash in visible content: ${sourcePath}`);
    }
    if (relativePath.endsWith('.json')) JSON.parse(source);
  }
}

for (const definition of versionDefinitions) {
  for (const locale of locales) {
    const generatedFiles = (
      await collectFiles(path.join(generatedRoot, definition.id, locale))
    )
      .map((file) =>
        path.relative(path.join(generatedRoot, definition.id, locale), file),
      )
      .filter((file) => allowedExtensions.has(path.extname(file)))
      .sort();
    assertSameList(
      chineseFiles,
      generatedFiles,
      `${definition.id}/${locale} generated paths`,
    );

    for (const relativePath of generatedFiles) {
      const generated = await readFile(
        path.join(generatedRoot, definition.id, locale, relativePath),
        'utf8',
      );
      if (/\{\{[A-Z0-9_]+\}\}/.test(generated)) {
        throw new Error(
          `Unresolved token in ${definition.id}/${locale}/${relativePath}`,
        );
      }
      if (/<!--\s*@(if|unless|endif)\b/.test(generated)) {
        throw new Error(
          `Unresolved conditional in ${definition.id}/${locale}/${relativePath}`,
        );
      }
    }
  }
}

console.log(
  `Verified ${pageFiles.length} pages in 2 locales across ${versions.length} versions.`,
);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) return collectFiles(absolutePath);
      return entry.isFile() ? [absolutePath] : [];
    }),
  );
  return files.flat();
}

function assertSameList(expected, actual, label) {
  if (JSON.stringify(expected) === JSON.stringify(actual)) return;

  const expectedSet = new Set(expected);
  const actualSet = new Set(actual);
  const missing = expected.filter((item) => !actualSet.has(item));
  const extra = actual.filter((item) => !expectedSet.has(item));
  throw new Error(
    `${label} differ. Missing: ${missing.join(', ') || 'none'}. Extra: ${extra.join(', ') || 'none'}.`,
  );
}
