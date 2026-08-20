import { access, readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defaultVersion, versions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const siteRoot = path.resolve(scriptDirectory, '..');
const outputRoot = path.join(siteRoot, 'doc_build');
const generatedRoot = path.join(siteRoot, '.generated-docs');
const base = (process.env.DOCS_BASE ?? '/ORM/').replace(/^\/+|\/+$/g, '');
const locales = ['zh', 'en'];

const sourcePages = (
  await collectFiles(path.join(generatedRoot, defaultVersion, 'zh'))
)
  .filter((file) => /\.(md|mdx)$/.test(file))
  .map((file) =>
    path.relative(path.join(generatedRoot, defaultVersion, 'zh'), file),
  );

for (const version of versions) {
  for (const locale of locales) {
    for (const page of sourcePages) {
      const route = outputRoute(version, locale, page);
      await access(path.join(outputRoot, route));
    }

    const prefix = routePrefix(version, locale);
    await access(path.join(outputRoot, prefix, 'llms.txt'));
    await access(path.join(outputRoot, prefix, 'llms-full.txt'));
  }
}

for (const asset of ['a3s-orm-mark.svg', 'favicon.svg', 'social-card.svg']) {
  await access(path.join(outputRoot, asset));
}

const defaultHome = await readFile(path.join(outputRoot, 'index.html'), 'utf8');
assertIncludes(defaultHome, 'lang="zh"', 'default homepage language');
assertIncludes(defaultHome, 'data-orm-home', 'custom ORM homepage');
assertIncludes(defaultHome, '简体中文', 'Chinese locale option');
assertIncludes(defaultHome, 'English', 'English locale option');
assertIncludes(defaultHome, 'aria-label="SQL 方言"', 'dialect tab list label');
assertIncludes(
  defaultHome,
  'aria-controls="orm-compiler-panel"',
  'dialect tab panel relationship',
);
assertIncludes(defaultHome, 'role="tabpanel"', 'dialect tab panel semantics');
for (const version of versions) {
  assertIncludes(defaultHome, version, `version option ${version}`);
}

const englishHome = await readFile(
  path.join(outputRoot, 'en', 'index.html'),
  'utf8',
);
assertIncludes(englishHome, 'lang="en"', 'English homepage language');
assertIncludes(englishHome, 'Typed queries', 'English homepage copy');

const nestedPage = await readFile(
  path.join(outputRoot, 'getting-started', 'overview.html'),
  'utf8',
);
assertIncludes(
  nestedPage,
  `/${base}/v0.2.0/getting-started/overview`,
  'version-aware nested-page link',
);

const htmlFiles = (await collectFiles(outputRoot)).filter((file) =>
  file.endsWith('.html'),
);
for (const file of htmlFiles) {
  const html = await readFile(file, 'utf8');
  if (/\{\{[A-Z0-9_]+\}\}/.test(html)) {
    throw new Error(`Unresolved documentation token in ${file}`);
  }
}

console.log(
  `Verified ${sourcePages.length * versions.length * locales.length} localized version routes and ${htmlFiles.length} HTML files.`,
);

function routePrefix(version, locale) {
  return [
    version === defaultVersion ? '' : version,
    locale === 'zh' ? '' : locale,
  ]
    .filter(Boolean)
    .join('/');
}

function outputRoute(version, locale, page) {
  const withoutExtension = page.replace(/\.(md|mdx)$/, '');
  const htmlPage = withoutExtension.endsWith('/index')
    ? `${withoutExtension.slice(0, -'/index'.length)}/index.html`
    : withoutExtension === 'index'
      ? 'index.html'
      : `${withoutExtension}.html`;
  return path.join(routePrefix(version, locale), htmlPage);
}

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

function assertIncludes(source, expected, label) {
  if (source.includes(expected)) return;
  throw new Error(`Missing ${label}: ${expected}`);
}
