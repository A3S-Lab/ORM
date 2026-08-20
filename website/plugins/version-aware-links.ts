import { readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import type { RspressPlugin, RouteMeta } from '@rspress/shared';
import {
  findPageByRoutePath,
  resolveVersionRoutePath,
  withDocsBase,
} from '../version-routing';

async function collectHtmlFiles(directory: string): Promise<string[]> {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = await Promise.all(
    entries.map(async (entry) => {
      const absolutePath = path.join(directory, entry.name);
      if (entry.isDirectory()) return collectHtmlFiles(absolutePath);
      return entry.name.endsWith('.html') ? [absolutePath] : [];
    }),
  );

  return files.flat();
}

function outputFileToRoutePath(outputRoot: string, file: string) {
  const relativePath = path
    .relative(outputRoot, file)
    .split(path.sep)
    .join('/')
    .replace(/\.html$/, '')
    .replace(/(^|\/)index$/, '');

  return `/${relativePath}`;
}

function replaceHref(anchor: string, href: string) {
  return anchor.replace(/\bhref="[^"]*"/, `href="${href}"`);
}

async function rewriteVersionLinks({
  base,
  defaultLang,
  defaultVersion,
  outputRoot,
  routes,
  versions,
}: {
  base: string;
  defaultLang: string;
  defaultVersion: string;
  outputRoot: string;
  routes: RouteMeta[];
  versions: string[];
}) {
  const htmlFiles = await collectHtmlFiles(outputRoot);
  const versionSet = new Set(versions);

  await Promise.all(
    htmlFiles.map(async (file) => {
      const currentPage = findPageByRoutePath(
        routes,
        outputFileToRoutePath(outputRoot, file),
      );
      if (!currentPage) return;

      const resolveHref = (targetVersion: string) =>
        withDocsBase(
          base,
          resolveVersionRoutePath(routes, currentPage, targetVersion, {
            defaultLang,
            defaultVersion,
          }),
        );
      const originalHtml = await readFile(file, 'utf8');
      const desktopVersionLink =
        /<a\b(?=[^>]*\bclass="[^"]*\brp-hover-group__item__link\b[^"]*")(?=[^>]*\baria-label="([^"]+)")[^>]*>/g;
      const mobileVersionLink =
        /<a\b(?=[^>]*\bclass="[^"]*\brp-nav-screen-versions-group__item\b[^"]*")[^>]*>([^<]+)<\/a>/g;
      const htmlWithDesktopLinks = originalHtml.replace(
        desktopVersionLink,
        (anchor, targetVersion: string) =>
          versionSet.has(targetVersion)
            ? replaceHref(anchor, resolveHref(targetVersion))
            : anchor,
      );
      const rewrittenHtml = htmlWithDesktopLinks.replace(
        mobileVersionLink,
        (anchor, label: string) => {
          const targetVersion = label.trim();
          return versionSet.has(targetVersion)
            ? replaceHref(anchor, resolveHref(targetVersion))
            : anchor;
        },
      );

      if (rewrittenHtml !== originalHtml) {
        await writeFile(file, rewrittenHtml);
      }
    }),
  );
}

export function versionAwareLinksPlugin(siteRoot: string): RspressPlugin {
  let routes: RouteMeta[] = [];

  return {
    name: 'a3s-version-aware-links',
    routeGenerated(generatedRoutes) {
      routes = generatedRoutes;
    },
    async afterBuild(config, isProd) {
      if (!isProd || routes.length === 0) return;

      const configuredOutput = config.outDir || 'doc_build';
      const outputRoot = path.isAbsolute(configuredOutput)
        ? configuredOutput
        : path.join(siteRoot, configuredOutput);

      await rewriteVersionLinks({
        base: config.base || '/',
        defaultLang: config.lang || '',
        defaultVersion: config.multiVersion?.default || '',
        outputRoot,
        routes,
        versions: config.multiVersion?.versions || [],
      });
    },
  };
}
