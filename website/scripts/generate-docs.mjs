import {
  cp,
  mkdir,
  readdir,
  readFile,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { versionDefinitions } from '../versions.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const siteRoot = path.resolve(scriptDirectory, '..');
const contentRoot = path.join(siteRoot, 'content');
const publicRoot = path.join(siteRoot, 'public');
const outputRoot = path.join(siteRoot, '.generated-docs');
const locales = ['zh', 'en'];

if (path.basename(outputRoot) !== '.generated-docs') {
  throw new Error(`Refusing to replace unexpected output path: ${outputRoot}`);
}

await rm(outputRoot, { force: true, recursive: true });
await mkdir(outputRoot, { recursive: true });

for (const definition of versionDefinitions) {
  for (const locale of locales) {
    const sourceDirectory = path.join(contentRoot, locale);
    const outputDirectory = path.join(outputRoot, definition.id, locale);
    await generateDirectory(sourceDirectory, outputDirectory, {
      ...definition,
      locale,
    });
  }
}

await cp(publicRoot, path.join(outputRoot, 'public'), { recursive: true });

async function generateDirectory(sourceDirectory, outputDirectory, context) {
  const entries = await readdir(sourceDirectory, { withFileTypes: true });
  await mkdir(outputDirectory, { recursive: true });

  for (const entry of entries) {
    const sourcePath = path.join(sourceDirectory, entry.name);
    const outputPath = path.join(outputDirectory, entry.name);

    if (entry.isDirectory()) {
      await generateDirectory(sourcePath, outputPath, context);
      continue;
    }

    if (!entry.isFile()) continue;

    const fileStats = await stat(sourcePath);
    if (fileStats.size > 2_000_000) {
      throw new Error(`Content source is unexpectedly large: ${sourcePath}`);
    }

    const source = await readFile(sourcePath, 'utf8');
    const generated = renderVersion(source, context);
    await writeFile(outputPath, generated, 'utf8');
  }
}

function renderVersion(source, context) {
  const capabilities = new Set(context.capabilities);
  let output = source;

  output = output.replace(
    /<!--\s*@if\s+([a-z0-9-]+)\s*-->([\s\S]*?)<!--\s*@endif\s*-->/gi,
    (_, capability, body) => (capabilities.has(capability) ? body : ''),
  );
  output = output.replace(
    /<!--\s*@unless\s+([a-z0-9-]+)\s*-->([\s\S]*?)<!--\s*@endif\s*-->/gi,
    (_, capability, body) => (capabilities.has(capability) ? '' : body),
  );

  const replacements = {
    DOC_VERSION: context.id,
    RELEASE: context.release,
    TAG: `v${context.release}`,
  };

  for (const [token, value] of Object.entries(replacements)) {
    output = output.replaceAll(`{{${token}}}`, value);
  }

  const unresolvedConditional = output.match(/<!--\s*@(if|unless|endif)\b/);
  if (unresolvedConditional) {
    throw new Error(
      `Unresolved conditional ${unresolvedConditional[0]} in ${context.id}/${context.locale}`,
    );
  }

  const unresolvedToken = output.match(/\{\{[A-Z0-9_]+\}\}/);
  if (unresolvedToken) {
    throw new Error(
      `Unresolved token ${unresolvedToken[0]} in ${context.id}/${context.locale}`,
    );
  }

  return output;
}
