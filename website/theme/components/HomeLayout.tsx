import { useState } from 'react';
import {
  ArrowRight,
  Check,
  Copy,
  Database,
  GitBranch,
  LockKey,
  Stack,
} from '@phosphor-icons/react';
import { useLang, useSite, useVersion, withBase } from '@rspress/core/runtime';
import { moveTabFocus, useCopyFeedback } from './HomeControls';

type Locale = 'zh' | 'en';
type Dialect = 'postgres' | 'sqlite' | 'mysql';

const dialects = ['postgres', 'sqlite', 'mysql'] as const;

const copy = {
  zh: {
    title: ['类型约束查询，', 'SQL 保持透明。'],
    body: '用 Rust 类型组合查询，由方言生成参数化 SQL，并通过异步 SQLite 或 PostgreSQL 执行。',
    start: '开始使用',
    queryGuide: '查看查询指南',
    compilerRegion: '交互式 SQL 编译器示例',
    dialectTabs: 'SQL 方言',
    query: '类型化查询',
    compiled: '编译结果',
    parameters: '绑定参数',
    unsupported:
      'MySQL 不支持这个 RETURNING 查询。A3S ORM 会明确拒绝，而不是近似执行。',
    installTitle: '选择运行时，复制一行依赖。',
    installBody: 'SQLite 默认启用。也可以只启用 PostgreSQL，或仅使用编译器。',
    copyCommand: '复制依赖配置',
    copied: '已复制',
    copyFailed: '复制失败',
    contractTitle: '查询结构由类型约束，执行边界保持显式。',
    contractBody: '从表声明到结果解码，每一层只负责一个稳定契约。',
    capabilities: [
      {
        title: '类型化结构',
        body: '表、列、值、赋值和结果形状在编译期保持一致。',
        code: 'orm_table!',
      },
      {
        title: '不可变查询 AST',
        body: '连接、CTE、窗口、集合运算和锁都保留为结构化节点。',
        code: 'Query',
      },
      {
        title: '执行器中立',
        body: '编译器只生成 SQL 与参数，异步驱动只处理连接和行。',
        code: 'Executor',
      },
      {
        title: '确定性迁移',
        body: '有序版本、SHA-256 校验和数据库级锁共同保护历史。',
        code: 'Migrator',
      },
    ],
    flowTitle: '一条查询只沿一个方向流动。',
    flowBody: '没有隐式持久化行为，也没有运行时值拼接进 SQL 文本。',
    flow: [
      ['表与表达式', 'Rust 类型定义合法组合'],
      ['查询 AST', '保留语义和参数边界'],
      ['方言编译器', '负责引号、占位符与能力检查'],
      ['异步执行器', '管理连接、事务与解码'],
    ],
    runtimesTitle: '同一套查询契约，两种生产运行时。',
    sqliteTitle: 'SQLite',
    sqliteBody: 'Tokio 安全的单连接执行器，默认 WAL、外键校验与取消安全事务。',
    sqlitePoints: ['默认 feature', '嵌套 savepoint', 'BEGIN IMMEDIATE 迁移锁'],
    postgresTitle: 'PostgreSQL',
    postgresBody:
      '有界连接池、预编译语句缓存、rustls、健康快照与明确的重试分类。',
    postgresPoints: [
      'Deadpool 连接池',
      '事务策略与 advisory lock',
      'TLS 连接池原子轮换',
    ],
    compare: '比较驱动与方言',
    boundariesTitle: '生产边界写在文档里，也写进错误路径。',
    boundaries: [
      '不支持的方言语法在编译阶段返回错误。',
      '迁移是只向前的，并校验已应用历史。',
      'SQLite 串行使用一个连接，PostgreSQL 使用有界池。',
      'MySQL 当前只提供 SQL 编译，不提供内置运行时。',
    ],
    readiness: '阅读生产就绪说明',
    ctaTitle: '从第一条可检查的查询开始。',
    ctaBody: '定义表，编译 SQL，再连接你选择的异步运行时。',
  },
  en: {
    title: ['Typed queries.', 'SQL stays visible.'],
    body: 'Compose with Rust types, compile parameterized SQL, then run it on async SQLite or PostgreSQL.',
    start: 'Get started',
    queryGuide: 'Read query guides',
    compilerRegion: 'Interactive SQL compiler example',
    dialectTabs: 'SQL dialect',
    query: 'Typed query',
    compiled: 'Compiled result',
    parameters: 'Bound parameters',
    unsupported:
      'MySQL does not support this RETURNING query. A3S ORM rejects it instead of approximating the behavior.',
    installTitle: 'Choose a runtime. Copy one dependency.',
    installBody:
      'SQLite is enabled by default. Select PostgreSQL or use the compiler alone.',
    copyCommand: 'Copy dependency',
    copied: 'Copied',
    copyFailed: 'Copy failed',
    contractTitle: 'Types constrain structure. Execution stays explicit.',
    contractBody:
      'Each layer owns one stable contract, from table declarations through decoded results.',
    capabilities: [
      {
        title: 'Typed structure',
        body: 'Tables, columns, values, assignments, and result shapes agree at compile time.',
        code: 'orm_table!',
      },
      {
        title: 'Immutable query AST',
        body: 'Joins, CTEs, windows, set operations, and locks remain structured nodes.',
        code: 'Query',
      },
      {
        title: 'Executor neutral',
        body: 'Compilers emit SQL and parameters. Async drivers own connections and rows.',
        code: 'Executor',
      },
      {
        title: 'Deterministic migrations',
        body: 'Ordered versions, SHA-256 checksums, and database locks protect history.',
        code: 'Migrator',
      },
    ],
    flowTitle: 'A query moves in one direction.',
    flowBody:
      'No implicit persistence behavior. No runtime value enters SQL text.',
    flow: [
      ['Tables and expressions', 'Rust types define legal composition'],
      ['Query AST', 'Semantics and parameter boundaries stay intact'],
      ['Dialect compiler', 'Owns quoting, placeholders, and capability checks'],
      ['Async executor', 'Owns connections, transactions, and decoding'],
    ],
    runtimesTitle: 'One query contract. Two production runtimes.',
    sqliteTitle: 'SQLite',
    sqliteBody:
      'A Tokio-safe single connection with WAL, foreign keys, and cancellation-safe transactions.',
    sqlitePoints: [
      'Default feature',
      'Nested savepoints',
      'BEGIN IMMEDIATE migration lock',
    ],
    postgresTitle: 'PostgreSQL',
    postgresBody:
      'A bounded pool with prepared statements, rustls, health snapshots, and explicit retry classes.',
    postgresPoints: [
      'Deadpool connection pool',
      'Transaction policy and advisory locks',
      'Atomic TLS pool rotation',
    ],
    compare: 'Compare drivers and dialects',
    boundariesTitle: 'Production boundaries live in docs and error paths.',
    boundaries: [
      'Unsupported dialect syntax fails during compilation.',
      'Migrations are forward-only and verify applied history.',
      'SQLite serializes one connection. PostgreSQL uses a bounded pool.',
      'MySQL currently compiles SQL without a bundled runtime.',
    ],
    readiness: 'Read production readiness',
    ctaTitle: 'Start with one inspectable query.',
    ctaBody:
      'Declare a table, compile its SQL, then connect the async runtime you need.',
  },
} as const;

const dialectOutput: Record<
  Dialect,
  { sql?: string; parameters?: string; error?: true }
> = {
  postgres: {
    sql: 'update "jobs"\nset "state" = $1\nwhere "jobs"."id" = $2\nreturning "jobs"."id"',
    parameters: '[String("leased"), I64(42)]',
  },
  sqlite: {
    sql: 'update "jobs"\nset "state" = ?\nwhere "jobs"."id" = ?\nreturning "jobs"."id"',
    parameters: '[String("leased"), I64(42)]',
  },
  mysql: { error: true },
};

function MarkdownHome({
  locale,
  version,
}: {
  locale: Locale;
  version: string;
}) {
  const text = copy[locale];
  return (
    <main>
      <h1>{text.title.join(' ')}</h1>
      <p>{text.body}</p>
      <h2>{text.installTitle}</h2>
      <pre>
        <code>{`a3s-orm = { git = "https://github.com/A3S-Lab/ORM", tag = "${version}" }`}</code>
      </pre>
      <h2>{text.contractTitle}</h2>
      {text.capabilities.map((item) => (
        <section key={item.title}>
          <h3>{item.title}</h3>
          <p>{item.body}</p>
        </section>
      ))}
      <h2>{text.flowTitle}</h2>
      <p>{text.flowBody}</p>
      <h2>{text.runtimesTitle}</h2>
      <p>{text.sqliteBody}</p>
      <p>{text.postgresBody}</p>
    </main>
  );
}

export function HomeLayout() {
  const locale: Locale = useLang() === 'zh' ? 'zh' : 'en';
  const text = copy[locale];
  const version = useVersion();
  const { site } = useSite();
  const [dialect, setDialect] = useState<Dialect>('postgres');
  const { copyStatus, copyText } = useCopyFeedback();
  const routePrefix = [
    version !== site.multiVersion.default ? version : '',
    locale !== site.lang ? locale : '',
  ]
    .filter(Boolean)
    .join('/');
  const route = (pathname: string) => {
    const normalized = pathname.replace(/^\/+/, '');
    return withBase(`/${[routePrefix, normalized].filter(Boolean).join('/')}`);
  };
  const installCommand = `a3s-orm = { git = "https://github.com/A3S-Lab/ORM", tag = "${version}" }`;
  const output = dialectOutput[dialect];

  if (import.meta.env.SSG_MD) {
    return <MarkdownHome locale={locale} version={version} />;
  }

  const copyInstallCommand = () => copyText(installCommand);

  return (
    <main className="product-home orm-home" data-orm-home>
      <section className="product-hero">
        <div className="product-hero__copy">
          <h1>
            {text.title.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </h1>
          <p>{text.body}</p>
          <div className="product-actions">
            <a
              className="product-button product-button--primary"
              href={route('/getting-started/quick-start')}
            >
              {text.start}
              <ArrowRight aria-hidden="true" size={17} weight="bold" />
            </a>
            <a
              className="product-button product-button--secondary"
              href={route('/queries/schema-and-expressions')}
            >
              {text.queryGuide}
            </a>
          </div>
        </div>

        <div
          aria-label={text.compilerRegion}
          className="compiler-demo"
          role="region"
        >
          <div
            aria-label={text.dialectTabs}
            className="compiler-demo__tabs"
            role="tablist"
          >
            {dialects.map((item, index) => (
              <button
                aria-controls="orm-compiler-panel"
                aria-selected={dialect === item}
                id={`orm-dialect-tab-${item}`}
                key={item}
                onClick={() => setDialect(item)}
                onKeyDown={(event) =>
                  moveTabFocus(event, dialects, index, setDialect)
                }
                role="tab"
                tabIndex={dialect === item ? 0 : -1}
                type="button"
              >
                {item === 'postgres'
                  ? 'PostgreSQL'
                  : item === 'sqlite'
                    ? 'SQLite'
                    : 'MySQL'}
              </button>
            ))}
          </div>
          <div className="compiler-demo__source">
            <span>{text.query}</span>
            <pre>
              <code>{`update_table::<Job>()
  .set(Job::state(), "leased")
  .filter(Job::id().eq(42))
  .returning(Job::id())`}</code>
            </pre>
          </div>
          <div
            aria-labelledby={`orm-dialect-tab-${dialect}`}
            aria-live="polite"
            className={`compiler-demo__output${output.error ? ' is-error' : ''}`}
            id="orm-compiler-panel"
            role="tabpanel"
          >
            <span>{text.compiled}</span>
            {output.error ? (
              <p>{text.unsupported}</p>
            ) : (
              <>
                <pre>
                  <code>{output.sql}</code>
                </pre>
                <div>
                  <strong>{text.parameters}</strong>
                  <code>{output.parameters}</code>
                </div>
              </>
            )}
          </div>
        </div>
      </section>

      <section className="install-rail">
        <div>
          <h2>{text.installTitle}</h2>
          <p>{text.installBody}</p>
        </div>
        <div className="install-command">
          <code>{installCommand}</code>
          <button
            aria-label={
              copyStatus === 'copied'
                ? text.copied
                : copyStatus === 'failed'
                  ? text.copyFailed
                  : text.copyCommand
            }
            onClick={copyInstallCommand}
            type="button"
          >
            {copyStatus === 'copied' ? (
              <Check aria-hidden="true" size={17} weight="bold" />
            ) : (
              <Copy aria-hidden="true" size={17} />
            )}
            <span aria-live="polite">
              {copyStatus === 'copied'
                ? text.copied
                : copyStatus === 'failed'
                  ? text.copyFailed
                  : text.copyCommand}
            </span>
          </button>
        </div>
      </section>

      <section className="product-section contract-section">
        <header className="section-heading">
          <h2>{text.contractTitle}</h2>
          <p>{text.contractBody}</p>
        </header>
        <div className="contract-grid">
          {text.capabilities.map((item, index) => {
            const Icon = [Stack, GitBranch, Database, LockKey][index];
            return (
              <article key={item.title}>
                <div className="contract-grid__icon">
                  <Icon aria-hidden="true" size={21} />
                </div>
                <code>{item.code}</code>
                <h3>{item.title}</h3>
                <p>{item.body}</p>
              </article>
            );
          })}
        </div>
      </section>

      <section className="product-section flow-section">
        <header className="section-heading">
          <h2>{text.flowTitle}</h2>
          <p>{text.flowBody}</p>
        </header>
        <ol className="query-flow">
          {text.flow.map(([title, body], index) => (
            <li key={title}>
              <span>{String(index + 1).padStart(2, '0')}</span>
              <h3>{title}</h3>
              <p>{body}</p>
            </li>
          ))}
        </ol>
      </section>

      <section className="product-section runtime-section">
        <header className="section-heading">
          <h2>{text.runtimesTitle}</h2>
        </header>
        <div className="runtime-comparison">
          <article>
            <div>
              <Database aria-hidden="true" size={24} />
            </div>
            <h3>{text.sqliteTitle}</h3>
            <p>{text.sqliteBody}</p>
            <ul>
              {text.sqlitePoints.map((point) => (
                <li key={point}>
                  <Check aria-hidden="true" size={15} weight="bold" />
                  {point}
                </li>
              ))}
            </ul>
          </article>
          <article>
            <div>
              <Stack aria-hidden="true" size={24} />
            </div>
            <h3>{text.postgresTitle}</h3>
            <p>{text.postgresBody}</p>
            <ul>
              {text.postgresPoints.map((point) => (
                <li key={point}>
                  <Check aria-hidden="true" size={15} weight="bold" />
                  {point}
                </li>
              ))}
            </ul>
          </article>
        </div>
        <a className="text-link" href={route('/execution/postgresql-and-ha')}>
          {text.compare}
          <ArrowRight aria-hidden="true" size={16} weight="bold" />
        </a>
      </section>

      <section className="product-section boundaries-section">
        <div>
          <h2>{text.boundariesTitle}</h2>
          <a
            className="text-link"
            href={route('/operations/architecture-and-production')}
          >
            {text.readiness}
            <ArrowRight aria-hidden="true" size={16} weight="bold" />
          </a>
        </div>
        <ul>
          {text.boundaries.map((boundary) => (
            <li key={boundary}>
              <Check aria-hidden="true" size={17} weight="bold" />
              <span>{boundary}</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="product-cta">
        <div>
          <h2>{text.ctaTitle}</h2>
          <p>{text.ctaBody}</p>
        </div>
        <a
          className="product-button product-button--primary"
          href={route('/getting-started/quick-start')}
        >
          {text.start}
          <ArrowRight aria-hidden="true" size={17} weight="bold" />
        </a>
      </section>
    </main>
  );
}
