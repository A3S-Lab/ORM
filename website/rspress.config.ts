import * as path from 'node:path';
import { defineConfig } from '@rspress/core';
import { versionAwareLinksPlugin } from './plugins/version-aware-links';
import { defaultVersion, versions } from './versions.mjs';

const base = process.env.DOCS_BASE ?? '/ORM/';
const siteOrigin = process.env.DOCS_ORIGIN ?? 'https://a3s-lab.github.io';

export default defineConfig({
  root: path.join(__dirname, '.generated-docs'),
  base,
  siteOrigin,
  title: 'A3S ORM',
  description:
    'Type-safe, executor-neutral SQL query building for Rust with async SQLite and PostgreSQL runtimes.',
  lang: 'zh',
  icon: '/favicon.svg',
  logo: '/a3s-orm-mark.svg',
  logoText: 'A3S ORM',
  outDir: 'doc_build',
  llms: true,
  route: {
    localeRedirect: 'never',
  },
  multiVersion: {
    default: defaultVersion,
    versions,
  },
  plugins: [versionAwareLinksPlugin(__dirname)],
  locales: [
    {
      lang: 'zh',
      label: '简体中文',
      title: 'A3S ORM',
      description:
        '面向 Rust 的类型安全、执行器中立 SQL 查询构建器，内置异步 SQLite 与 PostgreSQL 运行时。',
    },
    {
      lang: 'en',
      label: 'English',
      title: 'A3S ORM',
      description:
        'Type-safe, executor-neutral SQL query building for Rust with async SQLite and PostgreSQL runtimes.',
    },
  ],
  head: [
    ['meta', { name: 'theme-color', content: '#f7f7f8' }],
    ['meta', { property: 'og:type', content: 'website' }],
    ['meta', { property: 'og:site_name', content: 'A3S ORM' }],
    [
      'meta',
      {
        property: 'og:image',
        content: `${siteOrigin}${base}social-card.svg`,
      },
    ],
    ['meta', { name: 'twitter:card', content: 'summary_large_image' }],
    (route) => [
      'link',
      {
        rel: 'canonical',
        href: `${siteOrigin}${base.replace(/\/$/, '')}${route.routePath}`,
      },
    ],
  ],
  themeConfig: {
    search: true,
    enableContentAnimation: true,
    editLink: {
      docRepoBaseUrl:
        'https://github.com/A3S-Lab/ORM/tree/main/website/content',
    },
    lastUpdated: true,
    llmsUI: {
      placement: 'outline',
      viewOptions: ['markdownLink', 'chatgpt', 'claude'],
    },
    socialLinks: [
      {
        icon: 'github',
        mode: 'link',
        content: 'https://github.com/A3S-Lab/ORM',
      },
    ],
  },
});
