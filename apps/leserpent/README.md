# Leserpent Web Compatibility Bridge

> Community-facing product overview: [Leserpent](../../LESERPENT.md). This page
> retains implementation and compatibility-bridge details for contributors.

<p align="center">
  <img src="../../assets/branding/leserpent-icon.png" alt="Leserpent feathered serpent icon" width="220">
</p>

### Self-hosted Web renderer and 1.x migration surface

MIT License
Status: Active; follows the root `gewyvern` version line
Current shared release: `2.0.0`

Monorepo home:

* `apps/leserpent`
* works alongside `gewyvern` and `etragon` in the same repository

---

## 0. 定位

`apps/leserpent` 是：

> ASP.NET/TypeScript compatibility bridge and Web renderer

它不是 kernel runtime、eBPF execution engine，也不是 2.x control-plane
语义中心。

2.0 已经将 control-plane 语义迁移到 Rust `leserpentd`。Rust CLI、
Leselang、Avalonia、mobile 与 Web 都是同一 command/query contract 的
可替换入口：

* [Leserpent 2.0 architecture](../../docs/leserpent-2-architecture.md)
* [Leserpent 1.0 to 2.0 roadmap](../../docs/leserpent-2-roadmap.md)

这层负责：

* 承载自托管 Web 控制台
* 将兼容路由和旧状态迁移到 Rust contract
* 渲染 daemon 的权威 projection
* 保留迁移期恢复与审计兼容性
* 为 Windows 等尚无原生客户端的平台提供 Web 入口

---

## 1. 核心原则

1. 不直接操作 kernel
2. 不直接 attach eBPF
3. 所有网络调试 execution 委托 Gewyvern
4. 所有 control-plane 决策委托 Rust `leserpentd`
5. 必须依赖 runtime capability 与 expected revision
6. bridge 可以翻译，不可以建立第二权威
7. 当前核心能力全部采用 MIT 许可证、开源且免费；账号和订阅不得限制或回收既有能力

---

## 2. 部署模型

当前 2.x：

* Rust control runtime and native CLI
* separate local daemon and replaceable clients
* Avalonia desktop/mobile renderer
* authenticated web/mobile transport
* one client managing multiple independent daemon authorities
* one daemon authority managing multiple Gewyvern services

兼容 bridge：

* ASP.NET Core + TypeScript static frontend
* server-first, cross-platform, self-hosted
* 通过 Rust compatibility bridge 或 daemon transport 消费权威状态
* 不再新增只存在于 managed runtime 的 control-plane 语义

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
* 增量打包命令：`npm run package:frontend`
* 强制重建命令：`npm run package:frontend:force`
* 清单校验命令：`npm run verify:frontend-package`
* 原生入口：`cargo run --locked -p leserpent-frontend-package -- --verify`
* Release 热路径：MSBuild 仅按 Rust Inputs/Outputs 增量编译协调器，随后直接执行 native binary
* 类型检查命令：`npm run check:frontend`
* .NET 构建命令：`dotnet build src/Leserpent/Leserpent.csproj`

这样可以保持：

* 运行时仍然只消费静态 `wwwroot` 资源
* 不引入 bundler
* 不改变 ASP.NET Core 的部署模型
* Release 构建由 Rust 原生协调器在静态资源扫描前按内容哈希校验并按需重建；工具未变化时不启动 Cargo，资产未变化时不启动 Node，避免发布陈旧 `app.js`
* 发布后的静态资源通过 `MapStaticAssets` 使用预生成 Brotli/Gzip、ETag 和内容指纹
* `≤920px` 使用可展开 Fleet 筛选，`≤600px` 使用安全区底部导航与 `44px` 触控目标，并保留键盘 roving-tab 语义
* runtime 列表按面板自身宽度适配；可用宽度 `≤920px` 时保留表格语义但重排为字段卡片，行选择与上下文菜单同时支持键盘和触控
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

```text
Avalonia / mobile / Web / CLI / Leselang
  -> authenticated IPC or HTTPS/WebSocket
  -> leserpentd
  -> versioned Gewyvern machine contract
  -> one or more Gewyvern services
```

命令、查询和事件使用同一版本化语义。传输可以不同，但 transport 不拥有
policy；renderer 也不能绕过 daemon 直接调用部署或 Gewyvern adapter。

---

## 4. 信任模型

* 本地 authority 使用私有 Unix IPC 与 endpoint token
* 远程 authority 使用 HTTPS、显式 CA 信任和 endpoint-scoped token
* bootstrap credential 只用于安装和绑定 daemon，不自动升级为长期 authority
* Gewyvern deployment credential 只进入 capability-gated adapter
* desktop/mobile token 只进入平台 secret store，不进入 UI document 或 profile

---

## 5. Intent assembler

用户：

* 不接触 wire JSON
* 不写 JSON

UI：

* 组合 typed action、表单和 Orchestra plan

Rust authority：

> 校验 capability、identity、revision 与 confirmation，并生成可重放 command plan

---

## 6. 能力依赖模型

所有 Leserpent frontend：

* 必须读取 gewyvern capability
* 不可假设 runtime 能力
* 不可在 frontend 自行补出 daemon 未声明的操作

---

## 7. 三态处理

| 状态              | 行为           |
| --------------- | ------------ |
| unavailable | 禁止并提供稳定原因 |
| confirmation required | 生成预览并要求显式确认 |
| available | 提交 revision-fenced command |

---

## 8. session 管理

`leserpentd`：

* 创建
* 查询
* 停止
* 审计与重放

但：

> 不管理 kernel state

---

## 9. 安全责任

* `leserpentd` 承担 identity、capability、confirmation、revision 与 audit 权威
* Gewyvern 承担 runtime admin token、采集权限和 observed-truth 边界
* renderer 承担平台 secret storage，但不读取或持久化明文到 UI state
* Team Silvortex account 是可选 hosted-service 身份，不是开源核心能力门槛

---

## 10. UI 目标

* 多 daemon hub 与每个 daemon 的独立 workspace
* runtime、Orchestra、日志和 debugger projection
* GUI action 与 Leselang/CLI 一一对应
* renderer-neutral UI IR、可访问性和本地平台体验

---

## 11. 不做的事情

* 不做 kernel runtime
* 不做 attach
* 不做 verifier
* 不做 eBPF 编译
* 不在 C# 或 TypeScript 中新增 control-plane authority
* 不绕过 Leselang/domain protocol 建立 GUI 私有操作

---

## 12. 当前 bridge 实现

当前仓库保留一个成熟的 ASP.NET Core 兼容服务：

- `src/Leserpent`
  - standalone Web API service
  - built-in static dashboard
  - legacy runtime/session state import and recovery
  - daemon-authoritative runtime and Orchestra projection
  - capability-aware compatibility routes
  - static TypeScript dashboard

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
- `GET /v1/persistence/runtime-deletion-retry-audit`
- `GET /v1/persistence/runtime-deletions`
- `POST /v1/persistence/runtime-deletions/{intentId}/retry-now`
- `POST /v1/persistence/import`
- `POST /v1/persistence/save`
- `GET /v1/sessions`
- `GET /v1/sessions/{id}`
- `POST /v1/sessions`
- `POST /v1/sessions/{id}/stop`

当前这层故意保持为 bridge：

- 旧 runtime/session 状态继续支持导入、恢复和显式兼容路由
- 配置 daemon 后，Rust schema 与 writer fence 是 Orchestra 和 mutation 权威
- managed persistence 只保留离线兼容与迁移职责，不与 daemon 双写
- 新操作必须先进入 Rust domain/protocol，再由 Web 层消费
- 远程信任使用明确的 HTTPS CA/token contract，不再承诺未实现的 gRPC 或隐式配对

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
- control-plane JSON schema v5 保存待完成的 runtime 删除意图、稳定注销 command ID、mutation 前 replay-horizon floor 及逐意图重试元数据；schema v1-v4 在读取时保守升级，无法证明旧 mutation 边界的意图保持 fail-closed
- 删除意图必须在 daemon mutation 前严格落盘，daemon 和本地 registry 都完成后才会清除；失败使用 1/2/4/8/16/30 秒封顶退避，且只持久化固定安全失败码
- `GET /v1/persistence/runtime-deletions` 提供只读运维视图；尚未到期的 poison 不占用已到期健康意图的领取名额
- `POST /v1/persistence/runtime-deletions/{intentId}/retry-now` 要求当前 revision、唯一 requestId 和操作者；成功后立即唤醒恢复 worker，陈旧 revision 或复用冲突返回 `409`
- retry-now 审计最多按线性化顺序保留最新 256 条，严格落盘且在删除收敛后继续存在；窗口内 request ID 可幂等 replay，oldest-first 淘汰后旧操作不再可 replay 且该 ID 可用于新的 intent；`GET /v1/persistence/runtime-deletion-retry-audit` 提供只读查询
- 服务重启后，待删 runtime 继续拒绝新 session 和 Orchestra run，直到幂等 daemon 注销及本地清理收敛；state import 不接受待执行删除意图
- `scripts/validation/leserpent_runtime_deletion_crash.sh` 使用真实 Rust daemon 和独立 C# 子进程，在 daemon 提交后强杀宿主并验证重启收敛
- `scripts/validation/leserpent_runtime_deletion_fault_campaign.sh` 重复覆盖意图落盘、daemon 提交和本地清理三个持久化边界，并为当前平台保留聚合证据
- `scripts/validation/leserpent_runtime_deletion_concurrency_campaign.sh` 在同一故障战役中并发注册无关 runtime 和保存状态，验证删除恢复不会覆盖正常流量
- `scripts/validation/leserpent_runtime_deletion_daemon_restart_campaign.sh` 让第一次恢复在 daemon 离线时失败，再以同一数据库受控重启并验证重试收敛
- `scripts/validation/leserpent_runtime_deletion_unclean_takeover.sh` 强杀 daemon，验证 30 秒 owner lease 内拒绝双主、自然过期后同库接管与删除恢复收敛
- `scripts/validation/leserpent_runtime_deletion_overlapping_takeover.sh` 在一次非干净接管中恢复处于三个不同持久化边界的独立删除意图
- `scripts/validation/leserpent_runtime_deletion_repeated_takeover.sh` 在部分恢复提交后再次强杀 daemon，验证剩余意图可经第二次租约接管继续收敛
- `scripts/validation/leserpent_runtime_deletion_poison_isolation.sh` 让队首删除意图持续失败，验证健康意图不被饿死且毒化预约跨重载仍受保护
- `scripts/validation/leserpent_runtime_deletion_high_cardinality.sh` 运行 32-intent/4-poison 队列；恢复循环每批最多领取 32 个意图、并发执行 8 个 daemon mutation、每个 daemon tick 最多处理 64 个 IPC 连接，再以一次严格本地保存提交成功项
- `scripts/validation/leserpent_runtime_deletion_batch_persistence.sh` 在真实 daemon 注销成功后破坏本地严格批保存，验证全部内存投影回滚、预约继续受保护，并由下一轮幂等重放自动收敛
- `scripts/validation/leserpent_runtime_deletion_saturated_queue.sh` 填满 128-intent 队列，在 8 个阻塞槽下验证可取消停机，再以慢目标和 8 个 poison 验证四批健康收敛、逐意图持久退避、revision-fenced retry-now、跨收敛审计及修复收敛
- `scripts/validation/leserpent_runtime_deletion_retry_claim_race.sh` 以 worker-first、operator-first 和 32 轮同时起跑竞态验证 retry-now/claim 线性化、确定性冲突、持久审计及每个 runtime 仅一次权威删除
- `scripts/validation/leserpent_runtime_deletion_retry_crash.sh` 在 retry-now 严格落盘后以及真实 daemon 注销已提交后分别强杀宿主，验证 revision、审计和 pending intent 重启恢复、幂等 authority replay 及收敛后 request-ID replay
- `scripts/validation/leserpent_runtime_deletion_retry_rollover.sh` 以 `128/128/16` 三波并发 operator/worker 流量生成 272 条 retry 审计，验证 256 条 oldest-first 保留、单调线性化时间、明确 replay horizon、淘汰 ID 复用和无 pending 饥饿
- `scripts/validation/leserpent_runtime_deletion_retry_atomic_rollover.sh` 在 256 条满载审计的 rollover 写入前、真实临时文件创建后和原子提交后分别强杀宿主，逐条证明重启只能恢复完整旧窗口或完整新窗口
- `scripts/validation/leserpent_runtime_deletion_retry_atomic_backup.sh` 在备份刷新前、真实 `.bak.*.tmp` 创建后和主提交后分别强杀，再主动破坏主文件，验证回退始终恢复完整 256 条上一代审计
- `scripts/validation/leserpent_runtime_deletion_retry_post_recovery_write.sh` 从损坏主文件和完整备份启动，在首次修复写入前、主临时文件出现后及提交后分别强杀，验证活动状态只恢复完整旧/新窗口且良好备份从不被损坏主文件覆盖
- `scripts/validation/leserpent_runtime_deletion_retry_semantic_generation.sh` 使用 schema 兼容但包含非法持久失败码的主 generation 重复相同强杀矩阵，证明语义非法状态会回退到备份且永远不能晋升
- guided session 已创建但审计写入失败时返回 `503 orchestra_persistence_unavailable`，响应携带 `sessionId`，调用方不应盲目重试创建
- runtime 单删和批量清理会先在一个 SQLite 事务中删除对应 run/event；失败时返回 `503 runtime_delete_persistence_unavailable`，registry 和 session 保持不变
- control-plane JSON 状态保存会在进程内串行化，写入唯一临时文件并刷盘后再原子替换；并发请求不会共享或截断同一个 `.tmp` 文件
- control-plane 备份刷新同样使用独立临时文件、完整复制、刷盘和原子替换；刷新期间强杀且随后主文件损坏时，加载器仍只恢复完整上一代状态
- 从备份恢复后，首次保存会跳过旧主文件的备份刷新并直接原子安装新主文件；只有成功提交的主 generation 才能在后续保存中晋升为备份
- StateStore 和 Registry 共用状态语义验证器；除删除意图/retry 审计约束外，runtime/session 及 legacy Orchestra run ID 必须稳定且大小写不敏感唯一，每个 session 和 run 必须引用已注册 runtime；runtime/session 的必填文本、已知状态、单调时间、非负统计量和嵌套集合会在恢复前校验，capability/requirement 各限制为 256 条且键大小写不敏感唯一，sidecar memory 同样限制为 256 个唯一 slot；runtime/sidecar 状态来源必须符合 `unobserved`、成功或 `fetch_failed` 的时间与固定错误码姿态，远端异常、sidecar 报错和 memory 抓取失败不会把原始错误文本持久化，诊断文本也有明确长度和控制字符边界；Orchestra request ID 在各 runtime 内唯一，保留窗口内的 retry parent 还必须满足同 runtime/plan、终态、attempt `+1` 和单调时间，已被 retention 淘汰的 parent 仍允许作为历史边界；run lifecycle 只接受已知 active/terminal outcome，执行和完成时间不得倒退或来自未来，active run 不得伪装完成，step 列表非空引用且最多 256 条，同时兼容旧 terminal 缺失 `completedAt`；磁盘恢复、保存和显式导入都在任何投影替换、SQLite 迁移或 generation 晋升前 fail closed，并通过 `semantic_invalid` 暴露固定失败原因
- `/health` 和 `/v1/capabilities` 的 `persistence.load` 使用固定枚举报告 `empty|primary|backup|none` 来源、`empty|clean|recovered|failed` 结果及无路径失败码；成功备份恢复保持 persistence ready，同时明确标记为 degraded but operable
- control-plane 保存与 Orchestra store 故障在健康和 capabilities 响应中只暴露 `control_plane_state_save_failed` / `orchestra_store_operation_failed`，完整底层异常仅进入本机日志
- Orchestra run/event envelope 在 JSON 恢复、SQLite、leserpentd IPC、内存 store 和 authority 回读上共用语义门：operator/revision/step/event 文本有界且无控制字符，attempt 最多 1000000、step 最多 256 条，event 必须与 run 的身份和 outcome 精确一致且时间不早于执行/完成时间；authority 读取失败会阻止 legacy 覆盖，executor 原始异常只写本机日志，持久历史使用固定失败摘要
- retained Orchestra event history 必须以无 `fromOutcome` 的 origin 开始，后续 EventId 严格递增、记录时间不倒退、`fromOutcome` 精确承接上一条 `toOutcome` 且转换合法，最后 outcome/时间必须对应 run；旧 SQLite run 缺少 event 时启动会先补 `legacy_import`，active run 再追加 restart recovery，任何篡改回读均 fail closed，事件 API 返回固定 503
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
- `GET /v1/persistence/runtime-deletion-retry-audit`
- `GET /v1/persistence/runtime-deletions`
- `POST /v1/persistence/runtime-deletions/{intentId}/retry-now`
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
  --locked-mode
dotnet publish apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -r linux-x64 \
  --no-restore \
  -o artifacts/leserpent/linux-x64
```

locked restore 会同时校验共享 lock file 中的 `osx-arm64` 与 `linux-x64`
运行时图，因此必须保持 RID-neutral；只在后续 `--no-restore` publish 阶段选择目标 RID。

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
