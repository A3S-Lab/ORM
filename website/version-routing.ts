export type VersionedPage = {
  lang: string;
  routePath: string;
  version: string;
};

type VersionDefaults = {
  defaultLang: string;
  defaultVersion: string;
};

export function normalizeRoutePath(routePath: string) {
  const normalized = routePath
    .split(/[?#]/, 1)[0]
    .replace(/\.html$/, '')
    .replace(/\/+$/, '');

  return normalized.startsWith('/') ? normalized || '/' : `/${normalized}`;
}

export function getPureRoutePath(
  page: VersionedPage,
  { defaultLang, defaultVersion }: VersionDefaults,
) {
  const parts = normalizeRoutePath(page.routePath).split('/').filter(Boolean);

  if (page.version !== defaultVersion && parts[0] === page.version) {
    parts.shift();
  }
  if (page.lang !== defaultLang && parts[0] === page.lang) {
    parts.shift();
  }

  return `/${parts.join('/')}`;
}

export function findPageByRoutePath<T extends VersionedPage>(
  pages: T[],
  routePath: string,
) {
  const normalizedRoutePath = normalizeRoutePath(routePath);
  return pages.find(
    (page) => normalizeRoutePath(page.routePath) === normalizedRoutePath,
  );
}

export function resolveVersionRoutePath<T extends VersionedPage>(
  pages: T[],
  currentPage: T,
  targetVersion: string,
  defaults: VersionDefaults,
) {
  const pureRoutePath = getPureRoutePath(currentPage, defaults);
  const exactPage = pages.find(
    (page) =>
      page.version === targetVersion &&
      page.lang === currentPage.lang &&
      getPureRoutePath(page, defaults) === pureRoutePath,
  );

  if (exactPage) return exactPage.routePath;

  const fallbackParts = [
    targetVersion !== defaults.defaultVersion ? targetVersion : '',
    currentPage.lang !== defaults.defaultLang ? currentPage.lang : '',
  ].filter(Boolean);

  return fallbackParts.length > 0 ? `/${fallbackParts.join('/')}/` : '/';
}

export function withDocsBase(base: string, routePath: string) {
  const baseSegment = base.replace(/^\/+|\/+$/g, '');
  const normalizedBase = baseSegment ? `/${baseSegment}/` : '/';
  const normalizedRoute = routePath.replace(/^\/+/, '');

  return normalizedRoute
    ? `${normalizedBase}${normalizedRoute}`
    : normalizedBase;
}
