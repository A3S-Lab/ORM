<p align="center">
  <img src="https://raw.githubusercontent.com/A3S-Lab/ORM/main/assets/readme/hero.svg" width="100%" alt="A3S ORM 将类型化 Rust 模式和谓词转换为参数化 SQL 以用于异步 PostgreSQL 和 SQLite 执行">
</p>

<p align="center">
  <strong>Language / 语言:</strong>
  <a href="README.md">English</a> ·
  <a href="README.zh-CN.md">中文</a>
</p>

<p align="center">
  <strong>显式查询。编译时限制。异步 PostgreSQL 和 SQLite。</strong>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/ORM/actions/workflows/ci.yml"><img alt="CI status" src="https://github.com/A3S-Lab/ORM/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="https://github.com/A3S-Lab/ORM/releases/latest"><img alt="最新版本" src="https://img.shields.io/github/v/release/A3S-Lab/ORM?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=5b8cff"></a>
  <img alt="Rust 1.85 或更新版本" src="https://img.shields.io/badge/rust-1.85%2B-9b7bff?style=flat-square">
  <a href="LICENSE"><img alt="MIT 许可证" src="https://img.shields.io/badge/license-MIT-0b1020?style=flat-square"></a>
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/ORM/">文档</a> ·
  <a href="#the-contract">契约</a>·
  <a href="#quick-start">快速入门</a>·
  <a href="#capability-map">功能</a>·
  <a href="#drivers-and-dialects">驱动程序</a> ·
  <a href="#migrations">迁移</a> ·
  <a href="#architecture">架构</a>
</p>

---

**A3S ORM** 是一个类型安全、执行器中立的 Rust SQL 查询构建器，
灵感来自 [Kysely](https://kysely.dev/)。表声明约束
编译时的列、值、赋值和解码结果。不可变的
构建器编译成 SQL 加绑定参数，然后通过异步执行
驱动程序中立的接口。

尽管有这个名字，但这并不是一个 Active Record 框架。记录不属于自己
持久性行为，查询保持可见，并且运行时值永远不会
插入到生成的 SQL 中。

[文档网站](https://a3s-lab.github.io/ORM/)提供了完整的
v0.3.1、v0.3.0 和 v0.2.0 的中英文指南，包括同页
语言和版本切换。

## 契约

定义一次架构，与类型化列组合，并检查确切的查询
在到达连接之前：

```rust
use a3s_orm::{orm_table, select_from, OrderDirection, PostgresDialect, Query};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
    }
}

fn main() -> Result<(), a3s_orm::Error> {
    let query = select_from::<Person>()
        .select((Person::id(), Person::name()))
        .filter(Person::age().gte(18))
        .order_by(Person::name(), OrderDirection::Asc)
        .limit(20)
        .compile(&PostgresDialect)?;

    println!("sql = {}", query.sql);
    println!("parameters = {:?}", query.parameters);
    assert_eq!(query.parameters.len(), 2);
    Ok(())
}
```

```text
sql = select "person"."id", "person"."name" from "person" where ("person"."age" >= $1) order by "person"."name" asc limit $2
parameters = [I64(18), U64(20)]
```

编译前会检查列所有权和 Rust 值族。的
dialect 拥有引用、占位符和功能支持；不支持的语法是
拒绝而不是近似。

## 快速开始

### 安装

SQLite 是默认运行时：

```toml
[dependencies]
a3s-orm = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

改为启用捆绑的 PostgreSQL 驱动程序：

```toml
a3s-orm = { version = "0.3", default-features = false, features = ["postgres"] }
```

或者使用查询生成器和方言编译器而不捆绑运行时：

```toml
a3s-orm = { version = "0.3", default-features = false }
```

版本"0.2.1"是仅限于以下应用程序的维护向后移植
`0.2` 兼容性线。新集成应使用"0.3"。

`postgres` 功能包括 UUID、JSON/JSONB、Chrono 日期/时间类型、
`rust_decimal::Decimal` 和 `SqlArray<T>`。

### 执行类型化的 SQLite 往返

对于真正的内存数据库来说，默认功能已经足够了：

```rust
use a3s_orm::{
    insert_into, orm_table, select_from, Database, SqliteDialect, SqliteExecutor,
};

orm_table! {
    struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = SqliteExecutor::open_in_memory().await?;
    executor
        .execute_schema(
            "create table person (id integer primary key, name text not null)",
        )
        .await?;

    let database = Database::new(SqliteDialect, executor);
    database
        .execute(
            insert_into::<Person>()
                .value(Person::id(), 1)
                .value(Person::name(), "Ada"),
        )
        .await?;

    let name: String = database
        .fetch_one_as(
            select_from::<Person>()
                .select(Person::name())
                .filter(Person::id().eq(1)),
        )
        .await?;

    assert_eq!(name, "Ada");
    Ok(())
}
```

## 能力图

- **类型化结构** - 模式标记约束列、连接、过滤器、
  插入、更新和结果解码。
- **可组合 SQL** — 不可变的 SELECT、INSERT、UPDATE 和 DELETE 构建器
  涵盖连接、CTE、"UPDATE FROM"、类型化表达式赋值、子查询、
  聚合、窗口、集合操作、函数、强制转换和冲突处理。
- **显式并发** — PostgreSQL 行锁、表锁、咨询锁、
  事务隔离、访问模式和超时仍然是类型化操作。
- **检查结果** — 标量、元组、可为空、数组、UUID、JSON、时间、
  和十进制值通过检查的转换进行解码。
- **取消安全执行** - 范围内的 SQLite 和 PostgreSQL 事务
  保留它们的连接，直到回滚清理完成。
- **确定性迁移** — 有序、校验和的迁移以原子方式运行
  在有界数据库锁后面。
- **受控逃生舱口** - `sql_query::<Output>` 接受经过审查的静态
  SQL 而动态值仍然通过"bind"进入。

### PostgreSQL 工作队列保持输入状态

锁定子句、CTE、更新源和表达式赋值是 AST 节点
而不是附加的 SQL 字符串。这使得候选人的选择和租约得以保留
在一个参数化语句中获取：

```rust
use a3s_orm::{
    orm_table, select_from, update_table, OrderDirection, PostgresDialect, Query,
};

orm_table! {
    struct Job => "jobs" {
        id: i64 => "id",
        state: String => "state",
        attempt_count: i32 => "attempt_count",
    }
}

orm_table! {
    struct JobCandidate => "job_candidate" {
        id: i64 => "id",
    }
}

fn main() -> Result<(), a3s_orm::Error> {
    let candidates = select_from::<Job>()
        .select(Job::id())
        .filter(Job::state().eq("ready"))
        .order_by(Job::id(), OrderDirection::Asc)
        .limit(1)
        .for_update_of::<Job>()
        .skip_locked()
        .as_cte::<JobCandidate>();
    let query = update_table::<Job>()
        .with(candidates)
        .set(Job::state(), "leased")
        .set_expression(Job::attempt_count(), Job::attempt_count() + 1)
        .from::<JobCandidate>()
        .filter(Job::id().eq_column(JobCandidate::id()))
        .returning((Job::id(), Job::attempt_count()))
        .compile(&PostgresDialect)?;

    assert!(query.sql.contains("for update of \"jobs\" skip locked"));
    assert!(query.sql.contains("update \"jobs\""));
    assert!(query.sql.contains("from \"job_candidate\""));
    Ok(())
}
```

事务范围的"advisory_xact_lock(namespace, key)"涵盖逻辑
还没有行的资源。重试分类标识
序列化、死锁、锁争用、故障转移、连接丢失和池
饱和而不自动重放写入。参见
[PostgreSQL HA 控制](docs/postgres-ha.md) 了解完整的契约。

## 驱动程序和方言

|能力| PostgreSQL | SQLite | MySQL |
| ---| :---: | :---: | :---: |
| SQL编译 |是的 |是的 |是的 |
|捆绑的异步驱动程序 |是的 |是的 |没有 |
| ‘回归’ |是的 |是的 |被拒绝 |
| 《关于冲突》|是的 |是的 |被拒绝 |
| `更新自` |是的 |是的 |被拒绝 |
|行锁和表锁|是的 |被拒绝 |被拒绝 |
|交易 |是的 |是的 | — |
|锁定迁移 |咨询锁| `立即开始` | — |
| UUID、JSON、时间、十进制、数组 |是的 | SQLite 原生子集 | — |

**SQLite** 使用 Tokio 安全的单连接。文件数据库默认为WAL，
外键强制执行，以及五秒的繁忙超时。嵌套保存点和
范围事务可以防止后续工作与取消清理竞争。

**PostgreSQL** 使用带有准备语句缓存的有界死池池。
驱动程序公开类型化交易策略、稳定的无标签健康指标、
重试分类、验证 rustls 连接和健康门控原子 TLS
池轮换。 `connect_no_tls` 用于本地或单独安全
连接。

MySQL 支持当前仅意味着 SQL 生成。它并不意味着捆绑
运行时驱动程序。阅读[生产准备情况](docs/development-readiness.md)
准确的部署范围和限制。

## 迁移

迁移按版本排序，使用 SHA-256 进行校验和，并记录在
`a3s_orm_migrations`。重新运行未更改的集合是无操作的；修改或
删除已应用的迁移是一个错误。

```rust
use a3s_orm::{Migration, Migrator, SqliteExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = SqliteExecutor::open_in_memory().await?;
    let report = Migrator::new(executor)
        .run([Migration::new(
            "001",
            "create people",
            "create table person (id integer primary key, name text not null)",
        )])
        .await?;

    assert_eq!(report.applied, vec!["001"]);
    Ok(())
}
```

故意没有 DDL 权限的服务进程使用相同的
通过"Migrator::verify_required"清单和分类帐。该调用不执行
表创建、锁获取或分类帐写入：每个提供的迁移
必须已经存在及其精确的校验和。其他应用的迁移是
承认，以便较旧的二进制文件可以显式参与
扩展兼容滚动升级。

```rust
# use a3s_orm::{Migration, Migrator, SqliteExecutor};
# async fn admit(executor: SqliteExecutor) -> Result<(), Box<dyn std::error::Error>> {
Migrator::new(executor)
    .verify_required([Migration::new(
        "001",
        "create people",
        "create table person (id integer primary key, name text not null)",
    )])
    .await?;
# Ok(())
# }
```

SQLite 通过其连接门协调迁移器
"立即开始"。 PostgreSQL 使用事务范围的咨询锁
有期限。迁移 SQL 及其历史条目以原子方式提交。

## 架构

查询API不依赖于数据库客户端：

```text
typed schema + expressions
          │
    immutable query AST
          │
     dialect compiler
          │
  SQL + bound parameters
          │
 async Executor / driver
```

编译器永远不会打开连接，驱动程序也永远不需要理解
类型化的构建器状态。一种新的方言实现了"Dialect"；新的运行时
实现"执行器"。模块见[架构](docs/architecture.md)
所有权和扩展规则。

## 生产边界

该库使不受支持的行为变得可见，而不是默默地坠落
背部：

- 捆绑的 SQLite 执行器在一个连接上序列化工作；
- MySQL 有编译器，但没有捆绑运行时驱动程序；
- 迁移是向前的；
- 标量函数和转换结果类型是显式调用者断言；
- 类型化 DDL 构建器、查询插件、自定义 PostgreSQL 域编解码器以及
  尚未包括模式代码生成。

在部署之前查看[生产准备情况](docs/development-readiness.md)，
[PostgreSQL HA 控制](docs/postgres-ha.md) 用于池和故障转移策略，以及
计划工作的[路线图](docs/roadmap.md)。

## 发展

该测试套件运行真实的 SQLite 数据库和 PostgreSQL 17 服务。 CI 检查
特征矩阵、编译失败文档测试、Rust 1.85 MSRV、严格的 Clippy、
无警告 rustdoc、依赖项建议和至少 90% 的行覆盖率。

```bash
cargo fmt --all -- --check
cargo test --no-default-features
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

要在本地运行 PostgreSQL 集成测试：

```bash
A3S_ORM_POSTGRES_URL=postgres://postgres:postgres@127.0.0.1:5432/a3s_orm \
  cargo test --all-features
```

版本化的双语文档站点位于"website/"：

```bash
cd website
npm ci
npm run check
npm run build
npm run check:site
```

## 许可证

[麻省理工学院许可证](https://github.com/A3S-Lab/ORM/blob/main/LICENSE)