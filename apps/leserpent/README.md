# leserpent Design Spec

<p align="center">
  <img src="../../assets/branding/leserpent-icon.png" alt="Leserpent feathered serpent icon" width="220">
</p>

### eBPF Control Plane & Visual Orchestration Service

MIT License
Status: Active; follows the root `gewyvern` version line

Monorepo home:

* `apps/leserpent`
* works alongside `gewyvern` and `etragon` in the same repository

---

## 0. 定位

leserpent 是：

> control plane / management service

当前 1.x 不是 CLI wrapper、kernel runtime 或 eBPF execution engine。

2.0 目标不是继续扩张这个 ASP.NET 应用，而是将 control-plane 语义迁移
到 Rust `leserpentd`，并让 Rust CLI、Leselang 与 Avalonia GUI 成为同一
command/query contract 的可替换入口：

* [Leserpent 2.0 architecture](../../docs/leserpent-2-architecture.md)
* [Leserpent 1.0 to 2.0 roadmap](../../docs/leserpent-2-roadmap.md)

它负责：

* 生成 pipeline spec
* 分发
* 管理
* 可视化
* 审计

---

## 1. 核心原则

1. 不直接操作 kernel
2. 不直接 attach eBPF
3. 所有 execution 委托 gewyvern
4. 必须依赖 runtime capability
5. runtime 权威优先

---

## 2. 部署模型

当前 1.x：

* ASP.NET Core
* server-first
* 可跨平台部署
* 不建议部署普通客户端

目标 2.0：

* Rust control runtime and native CLI
* separate local daemon and replaceable clients
* Avalonia desktop/mobile renderer
* authenticated web/mobile transport
* current ASP.NET/TypeScript surface as the migration bridge

### Runtime posture

`leserpent` 的运行时目标是：

* 跨平台
* 尽量纯用户态
* 尽量少外部依赖
* 在 runtime / sidecar 全部降级时自己仍然可操作

更完整的运行边界见：

* `docs/runtime-posture.md`
* `docs/frontend-layout-maintenance.md`

### Frontend source layout

`leserpent` 的前端现在采用“TypeScript 源码 + 静态输出”的轻量模式：

* 源码入口：`src/Leserpent/frontend/app.ts`
* 生成产物：`src/Leserpent/wwwroot/app.js`
* 构建命令：`npm run build:frontend`
* 类型检查命令：`npm run check:frontend`
* .NET 构建命令：`dotnet build src/Leserpent/Leserpent.csproj`

这样可以保持：

* 运行时仍然只消费静态 `wwwroot` 资源
* 不引入 bundler
* 不改变 ASP.NET Core 的部署模型
* 后续可以逐步把现有脚本从宽松 JS 收紧成更完整的 TS

### Frontend layout maintenance

控制面板前端现在同时追求三件事：

* 单页 shell 导航
* 在较小窗口中尽量首屏可用
* deep-link 可以稳定恢复到指定 tab / pane / runtime

维护这部分时，优先参考：

* `docs/frontend-layout-maintenance.md`
* `docs/runtime-window-workspace.md`
* `docs/language-packs.md`

---

## 3. 连接模型

```
leserpent → gewyvern
```

短连接：

* 即时 gRPC

非：

* 长连接 agent 控制

---

## 4. 配对模型

流程：

1. gewyvern 提供 token
2. leserpent 生成密钥对
3. 交换公钥
4. 建立 trust

之后：

* 所有请求签名

---

## 5. Pipeline assembler

用户：

* 不接触 protobuf
* 不写 JSON

UI：

* 组合 pipeline

leserpent：

> 生成 protobuf spec

---

## 6. 能力依赖模型

leserpent：

* 必须读取 gewyvern capability
* 不可假设 runtime 能力

---

## 7. 三态处理

| 状态              | 行为           |
| --------------- | ------------ |
| not supported   | 禁止           |
| risky           | UI提示，不允许远程执行 |
| fully supported | 允许           |

---

## 8. session 管理

leserpent：

* 创建
* 查询
* 停止
* 审计

但：

> 不管理 kernel state

---

## 9. 安全责任

身份：

* user auth
* RBAC
* audit

runtime：

* 不承担

---

## 10. UI 目标

* pipeline 可视化
* session 可视化
* metrics / trace 展示

---

## 11. 不做的事情

* 不做 kernel runtime
* 不做 attach
* 不做 verifier
* 不做 eBPF 编译

---

## 12. 当前代码骨架

当前仓库已经有一个最小 ASP.NET Core control-plane 骨架：

- `src/Leserpent`
  - standalone Web API service
  - built-in static dashboard
  - in-memory runtime registry
  - capability-aware session creation
  - session query / stop surface

当前公开路由：

- `GET /health`
- `GET /v1/capabilities`
- `GET /v1/fleet/summary`
- `GET /v1/fleet/attention-summary`
- `POST /v1/fleet/refresh-all`
- `POST /v1/fleet/refresh-capabilities`
- `POST /v1/fleet/refresh-status`
- `GET /v1/fleet/runtimes-needing-attention`
- `GET /v1/runtimes`
- `GET /v1/runtimes/{id}`
- `GET /v1/runtimes/{id}/attention`
- `GET /v1/runtimes/{id}/status`
- `POST /v1/runtimes/register`
- `POST /v1/runtimes/{id}/refresh-capabilities`
- `POST /v1/runtimes/{id}/refresh-status`
- `GET /v1/persistence/export`
- `POST /v1/persistence/import`
- `POST /v1/persistence/save`
- `GET /v1/sessions`
- `GET /v1/sessions/{id}`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/stop`

当前这层故意保持很轻：

- 现在已经能主动抓取 gewyvern `/v1/capabilities`
- runtime registry / session state 现在会持久化到本地 state file
- 还没有真实 gRPC runtime client
- 还没有 pairing / signing
- 还没有 RBAC；Orchestra audit persistence 可由 managed SQLite 或配置后的 Rust daemon 提供

### 当前持久化行为

当前 `leserpent` 使用双层持久化：

- JSON 保存 runtime/session 控制状态，并继续作为导入、导出和灾难恢复格式
- 未配置 daemon 时，managed SQLite 保存 Orchestra run、审批、状态迁移结果和幂等 request ID
- 配置 daemon socket/token 后，Rust schema v10 成为 Orchestra 唯一权威，managed SQLite store 不会实例化或双写

JSON state 默认路径：

- 默认路径：
  - OS local application data 下的 `leserpent/control-plane-state.json`
- very-light backup:
  - 同目录的 `control-plane-state.json.bak`
- 可用环境变量覆盖：
  - `LESERPENT_STATE_PATH=target/leserpent/control-plane-state.json`
- SQLite 默认路径：
  - OS local application data 下的 `leserpent/control-plane.db`
- 可用环境变量覆盖：
  - `LESERPENT_DATABASE_PATH=target/leserpent/control-plane.db`
- SQLite 使用 WAL、`synchronous=NORMAL` 和 5 秒 busy timeout
- SQLite schema v2 同时保存最新 run 快照和 append-only 状态事件；v1 数据库会在启动时原地升级
- Rust schema v10 原子保存 run/event、限制每 runtime 32 个 runs、约束 request ID 唯一性，并提供有界 history/delete IPC
- `GET /v1/orchestra/runtimes/{id}/runs/{runId}/events` 按顺序返回单次运行的审计时间线；旧数据在首次新状态转换前可能没有事件
- Orchestra 状态转换只有在 SQLite 快照与事件同时提交后才会发布到内存；数据库拒写时不会启动自动执行
- state import 的 Orchestra 批量替换失败会返回 `503 persistence_import_unavailable` 并恢复导入前的内存 registry
- guided session 已创建但审计写入失败时返回 `503 orchestra_persistence_unavailable`，响应携带 `sessionId`，调用方不应盲目重试创建
- runtime 单删和批量清理会先在一个 SQLite 事务中删除对应 run/event；失败时返回 `503 runtime_delete_persistence_unavailable`，registry 和 session 保持不变
- control-plane JSON 状态保存会在进程内串行化，写入唯一临时文件并刷盘后再原子替换；并发请求不会共享或截断同一个 `.tmp` 文件
- runtime 存在 `queued`/`running` Orchestra run 时，单删和批量删除会返回 `409 runtime_delete_orchestra_active` 及 `activeRuns`；批量操作不会部分删除其他 idle runtime
- 仓库只保留 `src/Leserpent/data/control-plane-state.sample.json`，真实运行态 state 不应该提交。

### Rust compatibility bridge

Leserpent 1.x 可以选择让 runtime list 和 status refresh 响应经过 Rust 2.0
协议核校验：

```bash
cargo build -p leserpent-protocol --bin leserpent-compat-bridge
export LESERPENT_RUST_BRIDGE_BIN="$PWD/target/debug/leserpent-compat-bridge"
```

`LESERPENT_RUST_BRIDGE_BIN` 必须是现存可执行文件的绝对路径。可用
`LESERPENT_RUST_BRIDGE_TIMEOUT_MS` 设置 `100..30000` ms 的请求期限，默认
为 2000 ms。启用后 bridge 拒绝、超时或协议错位会返回
`502 compatibility_bridge_failed`，status refresh 不会在校验失败时写入
registry。`/health` 和 `/v1/capabilities` 的 `rust_compatibility_bridge`
adapter 状态会显示是否真正启用。

未配置 bridge 时 1.x 行为保持不变。Linux Native AOT 发布包包含该 bridge；
迁移部署前应阅读 [兼容策略](../../crates/leserpent-protocol/COMPATIBILITY.md)。

### leserpentd deployment authority

配置后的 1.x 部署入口可以把 Rust 规范化后的 intent 提交给 `leserpentd`，
等待其持久化 effect worker 完成，再用 typed receipt 还原现有 HTTP 响应：

```bash
export LESERPENT_IPC_TOKEN='replace-with-at-least-32-private-bytes'
export GEWY_API_ADMIN_TOKEN='the-gewyvern-admin-token'
target/debug/leserpentd \
  --database /var/lib/leserpent/runtime-authority.db \
  --socket /run/leserpent/leserpentd.sock \
  --gewyvern-target runtime-a=127.0.0.1:9411

export LESERPENT_DAEMON_SOCKET=/run/leserpent/leserpentd.sock
export LESERPENT_DAEMON_TOKEN="$LESERPENT_IPC_TOKEN"
export LESERPENT_DAEMON_DEPLOY_TIMEOUT_MS=5000
export LESERPENT_DAEMON_ORCHESTRA_TIMEOUT_MS=5000
```

`LESERPENT_DAEMON_SOCKET` 和 `LESERPENT_DAEMON_TOKEN` 必须同时设置。Socket
必须是绝对路径、不超过 100 UTF-8 bytes、不是符号链接且没有 group/other
权限；token 与 daemon 的 `LESERPENT_IPC_TOKEN` 相同。配置后 transport、
timeout、协议或 receipt 身份错误均失败关闭，不会回退到 C# 直连。Daemon
会把命令 ID 与 request ID 绑定，只允许读取 deployment effect 的终态回执。
首次配置的 Gewyvern target 会写入 Rust journal；以后启动时 endpoint 漂移会
拒绝启动，而不是静默覆盖 authority。

同一配置也会把 Orchestra store 切换到 Rust：保存、启动恢复、事件查询和
runtime history 删除均通过 owner-private IPC 完成；history 单页最多 64 条，
批量删除最多 128 个 runtime。任何 daemon、权限、超时、协议或 canonical
回读错误都失败关闭，不会在 managed SQLite 中留下隐藏的第二份权威状态。

未配置 daemon authority 时，1.x 保留原有 C# 直连部署路径。发布包包含
`leserpentd`，但安装器不会在 target、IPC token 和 Gewyvern secret 不完整时
自动启用它。`/health` 与 `/v1/capabilities` 中的
`leserpentd_deployment_authority` adapter 会显示实际启用状态。

当前会恢复和保存：

- registered runtimes
- discovered capabilities
- latest runtime status snapshots
- created sessions
- Orchestra runs、审批归属、状态和 request ID（所选 SQLite/Rust provider 为运行历史来源，JSON 保留灾备副本）

也就是说，重启 `leserpent` 后，runtime registry 和 session 列表不会重新变成空白。

如果所选 provider 为空、但旧 JSON 中存在 `orchestraRuns`，启动时会执行一次幂等导入。provider 已有数据时不会用 JSON 覆盖。

SQLite 不存储原始抓包、大型 eBPF 事件流或分析产物；这些内容仍应放在文件/对象存储中，数据库只保存编排审计和后续 artifact 索引。

### 当前 API 安全边界

当前 `leserpent` 已经补了一层 very-light 的 control-plane 安全边界：

- 默认 API 模式：
  - `loopback_only`
  - 也就是只有本机 loopback 请求可以直接访问控制面
- 如果设置：
  - `LESERPENT_ADMIN_TOKEN=...`
  - API 模式会变成：`loopback_or_token`
  - 远端请求必须带：
    - `X-Leserpent-Admin-Token: <token>`
- 对本地但敏感的写操作，还会要求 very-light intent header：
  - `X-Leserpent-Intent: mutate`
  - `X-Leserpent-Intent: export`
- dashboard 现在已经内建了一个 `Security` 小折叠区：
  - 可在浏览器本地保存 admin token
  - 之后所有控制面请求会自动带上 `X-Leserpent-Admin-Token`

同时，runtime / sidecar endpoint 的 server-side discovery 默认也已经收紧：

- 默认只允许 loopback 或私网地址
- 如果你明确要让控制面抓公网 endpoint，需要设置：
  - `LESERPENT_ALLOW_PUBLIC_ENDPOINTS=true`

当前 `/health` 和 `/v1/capabilities` 还会显式暴露 very-light runtime posture signals：

- `runtimePosture.coreReady`
- `runtimePosture.persistenceReady`
- `runtimePosture.degradedButOperable`
- `runtimePosture.optionalAdapters[]`

这层的目标不是把 adapter system 一次做满，而是先把控制面的核心姿态说清楚：

- 核心服务已经 ready
- persistence 现在是否 healthy
- 即使 persistence 降级，当前是否仍可操作
- 哪些能力只是 optional adapters，而不是启动前提

当前这些 very-light persistence signals 也会直接暴露出来：

- `GET /health`
  - `persistence.statePath`
  - `persistence.backupStatePath`
  - `persistence.lastSavedAt`
  - `persistence.schemaVersion`
  - `persistence.isDirty`
  - `persistence.lastSaveError`
  - `persistence.restoredRuntimeCount`
  - `persistence.restoredSessionCount`
  - `persistence.restoredFromSavedAt`
  - `orchestraPersistence.provider`
  - `orchestraPersistence.location`
  - `orchestraPersistence.schemaVersion`
  - `orchestraPersistence.lastError`
  - `orchestraPersistence.ready`
- `GET /v1/capabilities`
  - `persistence.enabled`
  - `persistence.statePath`
  - `persistence.backupStatePath`
  - `persistence.lastSavedAt`
  - `persistence.schemaVersion`
  - `persistence.isDirty`
  - `persistence.lastSaveError`
  - `persistence.restoredRuntimeCount`
  - `persistence.restoredSessionCount`
  - `persistence.restoredFromSavedAt`
  - `persistence.orchestraStoreProvider`
  - `persistence.orchestraStoreLocation`
  - `persistence.orchestraStoreLastError`
  - `persistence.orchestraStoreSchemaVersion`
- `GET /v1/persistence/export`
  - downloads the current control-plane state as JSON
- `POST /v1/persistence/import`
  - imports a compatible control-plane state JSON document
  - immediately persists the imported state and refreshes the in-memory registry
- `POST /v1/persistence/save`
  - very-light manual flush of current control-plane state

它的目标是先把最小控制面 contract 站住：

- leserpent 认识多个 gewyvern runtime
- leserpent 基于 runtime capability 决定 session 是否允许创建
- leserpent 读取 runtime latest-meta 来判断当前 snapshot/status
- risky / unsupported capability 不会被静默放过

### 当前 fleet summary 语义

现在还有一个 very-light 的总览入口：

- `GET /v1/fleet/summary`

它当前聚合这些控制面信号：

- `runtimeCount`
- `runtimesWithLatestSnapshot`
- `runtimesWithSummaryJson`
- `runtimesWithAnalysisJson`
- `runtimesWithExternalSidecarContext`
- `runtimesWithExternalEvidenceChainEnrichment`
- `runtimesWithExternalDiagnosticOpinion`
- `runtimesWithObservedStatus`
- `runtimesWithStatusFetchFailed`
- `snapshotKindCounts`
- `statusSourceCounts`
- `environmentCounts`
- `clusterCounts`
- `roleCounts`

这样 leserpent 已经能先回答：

- 当前接了多少个 gewyvern runtime
- 有多少个 runtime 已经有 latest snapshot
- 有多少个 runtime 已经能给 summary / analysis machine-facing surfaces
- 有多少个 runtime 当前带 sidecar context
- 有多少个 runtime 当前已经带 evidence-chain enrichment / diagnostic opinion
- 有多少个 runtime 当前已经被观测到 status，以及其中多少个 status fetch 已失败
- latest snapshot 更偏 `single` 还是 `scan`
- 当前 status source 更偏 `gewyvern-api`、`fetch_failed` 还是 `unobserved`
- 这些 runtime 分别属于哪些 environment / cluster / role

### 当前 fleet attention 语义

现在还有一个 very-light 的 attention 入口：

- `GET /v1/fleet/runtimes-needing-attention`
- `GET /v1/fleet/attention-summary`

它当前只列出值得优先下钻的 runtime，并给 very-light reasons，例如：

- `status_fetch_failed`
- `no_latest_snapshot`
- `no_analysis_json`

同时还会给 very-light 的 `severity`：

- `critical`
  - 当前主要表示 status fetch 已失败
- `warning`
  - 当前主要表示还没观测到 latest snapshot / analysis 面

`attention-summary` 则会继续把这一层 very-light 地聚成：

- `criticalCount`
- `warningCount`
- `reasonCounts`

### 当前 single-runtime attention 语义

单个 runtime 现在也有一个对称的 very-light 入口：

- `GET /v1/runtimes/{id}/attention`

它会返回：

- `needsAttention`
- `severity`
- `reasons`

健康节点当前会落成：

- `needsAttention: false`
- `severity: "none"`
- `reasons: []`

### 当前 fleet refresh 语义

现在还有一个 very-light 的批量状态刷新入口：

- `POST /v1/fleet/refresh-all`
- `POST /v1/fleet/refresh-capabilities`
- `POST /v1/fleet/refresh-status`

它们会对当前过滤范围内的已注册 runtime 逐个拉取：

- `gewyvern /v1/capabilities`
- `gewyvern /v1/latest/meta`

并分别返回：

- all refresh:
  - `refreshedCount`
  - `runtimes[]`
- capability refresh:
  - `refreshedCount`
  - `runtimes[]`
- status refresh:
  - `refreshedCount`
  - `runtimes[]`

它们也都支持和其他 fleet 入口一致的 filtering：

- `?environment=...`
- `?cluster=...`
- `?role=...`

它也支持和 runtime list / fleet summary 一样的：

- `?environment=...`
- `?cluster=...`
- `?role=...`

这个总览入口现在也支持 very-light filtering：

- `?environment=prod`
- `?cluster=alpha`
- `?role=edge`
- 也可以组合使用，例如：
  - `?environment=prod&cluster=alpha`

### 当前 runtime tagging 语义

`POST /v1/runtimes/register` 现在支持 very-light 的 runtime tags：

- `environment`
- `cluster`
- `role`

这些 tags 只用于：

- fleet 可视化
- grouping
- operator filtering 前置语义

目前还**不**代表真正的调度策略，也不会改变 capability gating 逻辑。

### 当前 runtime filtering 语义

`GET /v1/runtimes` 现在支持按 tags 做 very-light filtering：

- `?environment=prod`
- `?cluster=alpha`
- `?role=edge`
- 也可以组合使用，例如：
  - `?environment=prod&cluster=alpha`

这层 filtering 只是 control-plane 视角下的 runtime 浏览与分组，不代表真正的 placement / scheduling policy。

### 本地运行

```bash
cd apps/leserpent/src/Leserpent
dotnet run
```

启动后：

- dashboard:
  - `/`
- control-plane API:
  - `/v1/...`

#### Native AOT self-host

Leserpent 提供独立的 Native AOT 发布 profile。发布时必须指定目标 RID，产物为不依赖目标机器安装 .NET runtime 的 self-contained 原生服务：

```bash
dotnet restore apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -p:PublishAot=true \
  -r linux-x64 \
  --locked-mode
dotnet publish apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -r linux-x64 \
  --no-restore \
  -o artifacts/leserpent/linux-x64
```

ARM64 Linux 使用 `-r linux-arm64`。Native AOT 不支持从 macOS 直接交叉编译 Linux 产物，因此 Linux 发布应在对应 Linux 构建机或 CI runner 上执行。

发布目录会自动包含 Leserpent、`leserpent-compat-bridge`、`leserpentd`、Linux 安装器、
systemd unit 和环境模板。Cargo bridge 会在 Linux publish 阶段以 `--locked
--release` 构建；首次安装与后续原子升级使用同一条命令：

```bash
sudo artifacts/leserpent/linux-x64/deploy/install.sh
```

默认安装布局：

- 只读版本：`/opt/leserpent/releases/<release-id>`
- 当前版本：`/opt/leserpent/current`
- 配置与管理员 token：`/etc/leserpent/leserpent.env`
- 状态与 SQLite：`/var/lib/leserpent`
- systemd：`leserpent.service`

安装器会执行健康检查，失败时自动切回上一版本。完整的部署、升级、staging 和卸载说明见 [docs/deployment.md](docs/deployment.md)。
已安装主机还可以从任意有效 bundle 执行 `deploy/install.sh --rollback`，
显式交换保留的 `current`/`previous` 版本；配置和 SQLite 状态不随版本目录切换。

已配对的 gewyvern runtime 还支持结构化的认证直部署入口：Leserpent 使用内存中的 runtime token 提交幂等 deployment intent，并将结果写入 Orchestra 审计。当前状态边界、请求格式与安全约束见 [docs/remote-deployment.md](docs/remote-deployment.md)。

当前 dashboard 已经支持：

- tab-shell single-page layout
- 30 officially maintained locales: 8 built in and 22 downloadable `core-ui` packs
- downloadable/importable `leserpent.language-pack/v1` locales with same-origin catalog, SHA-256 verification, English fallback, and RTL support
- auto-follow browser language with manual override
- very-light runtime registration
- fleet summary / attention summary
- runtime list / attention list
- multi-runtime child-window workspace with one independently stateful window per gewyvern instance
- open-selected / open-all / close-one / close-all window lifecycle
- keyed iframe rendering and lazy loading so one runtime update does not reload sibling windows
- optional paired `etragon` sidecar child views
- runtime/sidecar source switch shell for the active child window
- active deep-link state persisted in URL and the full window set persisted in browser-local storage
- fleet refresh actions
- single-runtime detail inspection
- single-runtime refresh actions
- persistence export / import / save controls

多实例子窗口的操作方式、状态优先级、性能边界与维护入口见 [docs/runtime-window-workspace.md](docs/runtime-window-workspace.md)。

附加语言包的格式、安装边界、安全限制和发布流程见 [docs/language-packs.md](docs/language-packs.md)。

### 当前 runtime discovery 语义

`POST /v1/runtimes/register` 现在支持两种模式：

- 手工注册 capability
- `fetchCapabilities=true` 时主动抓取 `gewyvern /v1/capabilities`

同时也支持 very-light 的 nearby sidecar pairing：

- `sidecarEndpoint`

如果提供这条 endpoint，leserpent 会把这台 `gewyvern` 当成
“runtime + optional paired etragon sidecar”的单元来管理。

抓取成功后，leserpent 会把 gewyvern 的轻量 API surface 归一化成控制面可读的 capability，例如：

- `api.latest_snapshot`
- `api.target_routing`
- `api.analysis_json`
- `api.summary_json`
- `api.report_html`
- `api.external_sidecar_context`
- `runtime.serve_required`

这意味着现在 leserpent 已经开始真的依赖 runtime 权威，而不是只相信人工录入。

### 当前 runtime status 语义

除了 capability 抓取，leserpent 现在还会读取 `gewyvern /v1/latest/meta`，并缓存 very-light runtime status：

- `hasLatestSnapshot`
- `snapshotKind`
- `targetCount`
- `hasSummaryJson`
- `hasAnalysisJson`
- `hasExternalSidecarContext`
- `hasExternalEvidenceChainEnrichment`
- `hasExternalDiagnosticOpinion`

如果 runtime 还配置了 `sidecarEndpoint`，leserpent 也会去读取 paired
`etragon` 的 very-light sidecar status：

- `healthy`
- `daemonStatus`
- `learningActive`
- `learnedRoutes`
- `hasEvidenceChainEnrichment`
- `hasDiagnosticOpinion`

相关控制面入口现在包括：

- `POST /v1/runtimes/{id}/refresh-sidecar`
- `GET /v1/runtimes/{id}/sidecar`
- `POST /v1/fleet/refresh-sidecars`

`POST /v1/fleet/refresh-all` 现在也会在刷新 `gewyvern` capability/status 的同时，
顺手刷新已配对的 `etragon` sidecar 状态。
