---

# 🐉 gewyvern v0.03a — Template-Window Flow Debugger Runtime

### TCP Network Debugger Runtime (Engineering-first)

Status: Draft (0.03a)
Scope: **TCP-only**
Nature: Single-host / CLI-first / session runtime
Orientation: **network debugger（不是 observability）**

---

# 0. v0.03a 的“新增约束”是什么

v0.03 已定义了骨架。v0.03a 做三件事：

1. **把“模板驱动”与“窗口采集”变成运行时的第一公民**
2. **把“程序×网络流×栈阶段”的可感知建模加入 Flow View**（不是精确定位，强调“段落定位”）
3. **把 Reason / Confidence / Export 作为调试闭环的默认出口**（看不懂就导出问大模型）

v0.03a 不引入 ML，不引入业务解释，不引入 multi-host。

---

# 1. v0.03a 的唯一目标（更具体）

让 gewyvern 第一次成为：

> **模板化切面观测 + flow emergent + 可验证因果链 + 可控干预** 的调试器运行时

必须同时成立：

1️⃣ attach 成功（模板化 attach）
2️⃣ flow emergent 成功（TCP 生命周期连续体）
3️⃣ reason chain 可生成（L0→L1→L3）
4️⃣ intervention 能被 Gate 控制并执行（drop SYN/RST）

---

# 2. 用户问题范围（不变，但把输出形式固定）

v0.03a 只解决三类问题：

## 2.1 连接为什么失败

输出必须能回答：

* SYN 是否出现（哪个切面观测到）
* SYN-ACK 是否出现
* ACK 是否出现
* **断裂发生在：哪个“栈阶段段落”**（segment）

## 2.2 为什么卡顿 / 抖动

输出必须能回答：

* 是否有重传迹象（retrans evidence）
* 是否有路径变化迹象（route segment diverge）
* 是否有队列症状迹象（轻量证据，不做 queue engine）

## 2.3 为什么被断开

输出必须能回答：

* FIN / RST 的证据链
* 断开前生命周期演化
* 断开前路径段落变化证据

---

# 3. Runtime 架构（保持不变，新增 Template Runtime）

```
kernel plane (eBPF)
    ↓
Fact Stream
    ↓
Ledger (append-only)
    ↓
Flow Registry (projection)
    ↓
Reason Engine (lazy)
    ↓
Gate
    ↓
CLI Debugger
    ↓
Export (JSON / protobuf)
```

新增：**Template Runtime** 位于 attach 与 ingest_fact 之间，用于限定采集切面与时间窗口。

---

# 4. 设计原则（新增 2 条，保持原有不可变）

原有 4.1~4.5 不变，新增：

### 4.6 Template-first capture

**事实采集必须由模板定义。**
没有模板 = 不允许 attach。

### 4.7 Window-bounded runtime

**所有 session 都是 window-bounded 的。**
没有窗口 = 不允许启动 session（默认窗口可配置）。

> v0.03a 的核心：你不是在“长期观测”，你是在“开一次调试会话”。

---

# 5. Template Runtime（v0.03a 新核心）

## 5.1 Template 的定义

Template 是：

> “要抓什么事实（facts），在哪些切面抓（slices），用什么筛选（filters），用什么窗口拼装（window）”

Template 由四部分组成：

1. **Slices**：切面（栈阶段/观测点）
2. **Facts**：事实种类（TCP-only）
3. **Filters**：筛选规则（可为空）
4. **Window**：时间窗口策略（拼装与去乱序边界）

## 5.2 v0.03a 内置模板（不可扩展，先锁死）

v0.03a 内置 3 个模板，用户只能选其一（避免 DSL 爆炸）：

### T1: handshake_debug

* 目的：三次握手失败定位
* 必须采集：

  * TCP state transition
  * TCP packet meta（只含头部元信息）
  * route decision meta（简化）
* Window：

  * W = 5s（固定）

### T2: teardown_debug

* 目的：RST/FIN 断开定位
* 必须采集：

  * TCP packet meta（FIN/RST）
  * TCP state transition
  * route decision meta
* Window：

  * W = 10s（固定）

### T3: jitter_retrans_debug

* 目的：抖动/重传/路径变化粗定位
* 必须采集：

  * retrans evidence（迹象）
  * TCP packet meta（抽取 ACK/seq 摘要）
  * route decision meta
* Window：

  * W = 15s（固定）

> v0.03a 只做内置模板，0.04 才开放自定义模板/DSL。

## 5.3 Filters（v0.03a 允许的最小集合）

允许用户指定其中任意组合（全可选）：

* `tcp_key`: src/dst ip:port + direction
* `netns`
* `pid/cgroup`（若无法稳定获取则降级为 best-effort，不作为 identity 依据）
* `iface`（ifindex）

**强规则：Filter 只影响采集，不影响 flow identity。**

---

# 6. Facts（事实层）— 0.03a 固化 FactKind

Ledger 仍是“记录事实，不解释”。
v0.03a 固化以下事实种类：

### 必须

* `TCP_STATE_TRANSITION`
* `TCP_PACKET_META`
* `ROUTE_META`

### 可选（模板决定是否采集）

* `TCP_RETRANS_EVIDENCE`
* `SOCKET_LINEAGE_META`（可选，不进入 identity）

### 用户态事实（系统自举）

* `INTERVENTION_LOG`（执行记录）
* `SESSION_MARK`（start/stop marker）

每条事实必须带：

* ts, cpu, session_id, netns, ifindex, fact_kind

---

# 7. Window（窗口）— 0.03a 的拼装边界

窗口不是性能优化，是 correctness 约束：

* **flow 拼装只在 window 内进行**
* **window 外的事实不参与当前 flow view**
* window 结束后，Flow View 可以被冻结成 snapshot（可导出）

乱序处理规则：

* TCP_PACKET_META 在 window 内允许按 ts 排序拼装
* 不做 payload 重组
* 不做完整 seq-order reconstruction
* 只做 “握手/断开/重传/路径变更” 的行为连续性抽取

---

# 8. Flow Registry（runtime 核心）— 加入“段落定位”与“程序感知”

你原来的两层结构保持：

## 8.1 Flow Ledger（事实引用层）

* 只存 FactId 引用，不复制事实

## 8.2 Flow View（可变解释层）

新增字段（v0.03a）：

### lifecycle（TCP-only）

* handshake_phase
* established_phase（可选）
* teardown_phase

### path segments（段落）

* segment_id
* route_fingerprint（简化）
* ifindex/oif（若能拿到）
* segment_start_ts / end_ts
* diverge evidence refs

### program footprint（程序感知：best-effort）

* pid/comm/cgroup（若可获取）
* 仅用于“展示工作流程”，**不参与 identity**
* 允许为空

### evidence index

* retrans evidence list
* rst/fin evidence list
* “断裂点” evidence list

### anchor（延续）

* AnchorId：用于 diverge 新 flow 延续同一“语义实体”

---

# 9. Flow 定义（TCP-only，不变）

```
flow :=
  TCP lifecycle continuity
  + path continuity
  + time continuity (within window)
```

不依赖 PID/socket fd/iface。

---

# 10. Identity 匹配（实现规则更强调 window）

优先级不变，但加 window 约束：

1. (netns, sk_cookie) **且在同 window 内**
2. (netns, TcpKey + time continuity) **同 window 内连续**
3. TcpKey + packet 行为连续（握手片段连续）**同 window 内**

---

# 11. Segment / Diverge（段落与分流）— 0.03a 固化为“可定位即可”

### 11.1 Segment 的目标

不是精确定位路由/队列内部细节，而是：

> 给出“哪一段栈阶段/路径段落更可能异常”。

### 11.2 Diverge 规则（保持你的硬规则，但强调“概率性定位”）

path change = new flow（同 AnchorId）

触发条件（任选其一）：

* route fingerprint 改变，并持续超过 `T_diverge_min`
* 或 route fingerprint 改变的 evidence 次数 > `N`

结果：

* 新 FlowId
* 旧 flow 标记 diverged
* anchor 延续

---

# 12. Reason Chain（L0→L1→L3）— 增加“段落定位输出”

Reason 仍然 lazy，只允许 L0→L1→L3。

## L0（事实）

* packet/state/route/retrans

## L1（结构）

* timeline
* lifecycle
* **segments（必需）**
* breakpoints（断裂点：缺失/异常出现处）

## L3（叙事）

必须输出：

* 哪个阶段断裂
* 哪个 segment 更可疑
* evidence refs（可追溯）

> L3 不允许 AI 推理，不允许业务解释。

---

# 13. Confidence（无 ML，按“正常/不正常比”）

v0.03a 置信度定义为：

> abnormal evidence vs normal evidence 的比值（加上覆盖度）

* `E_abnormal`: retrans / rst / missing handshake step / route diverge
* `E_normal`: handshake completed / state progression / stable segment

输出：

* confidence ∈ [0,1]
* breakdown（展示 E_abnormal 与 E_normal 的计数与引用）

---

# 14. Intervention Model（不变，但把“模板闭环”写死）

## 决策单位

flow

## 执行单位

packet

## v0.03a 只支持

* drop SYN
* drop RST

**新增硬规则：**

* 只有当模板是 `handshake_debug` / `teardown_debug` 且 attach 闭环完整，才允许干预。

---

# 15. Safety Gate（不变，补充模板闭环条件）

必须满足：

1. session scope 存在
2. template 已选择（T1/T2/T3）
3. attach 闭环完整（模板要求的 FactKind 在 session 内出现）
4. reason chain 足够（L1 segments 已生成）
5. CLI confirm

否则拒绝。

---

# 16. CLI Contract（v0.03a 固定命令集 + 模板参数）

命令集保持你的列表，但 session start 增加模板：

* `session start --template {handshake_debug|teardown_debug|jitter_retrans_debug} --window {fixed}`
* `session stop`
* `attach tcp`（实际由 session start 隐式 attach，可保留显式命令但要幂等）
* `flow list`
* `flow inspect <id>`
* `reason show <id>`
* `drop syn <flow_id>`
* `drop rst <flow_id>`
* `export json --session <id> [--flow <id>]`
* `export pb  --session <id> [--flow <id>]`

---

# 17. Export-first（新闭环）

你说得对：看不懂就导出/截图问大模型。

因此 v0.03a 强制：

* `export json` 必须包含：

  * selected template
  * window params
  * flow snapshot（含 segments）
  * reason chain（含 refs）
  * intervention log（若发生）

JSON 目标是 “一 json 多吃”：

* CLI debug
* leserpent 元数据（未来 protobuf）
* 丢给 LLM 做解释（人类看不懂也行）

---

# 18. v0.03a 验收标准（保持 6 场景，补两条 export 条件）

你的 6 场景不变：

1. 正常三次握手
2. SYN 无 SYN-ACK
3. SYN-ACK 无 ACK
4. RST 断开
5. 丢包 → retrans 迹象
6. 路由变更 → diverge

全部必须：

* 生成 reason chain（含 segment）
* 可追溯事实（FactId refs）
* CLI 可展示

**新增：**

* 每个场景必须能导出 JSON 并可被外部工具/大模型读取解释
* 每个场景必须明确输出 “可疑段落/切面”（哪一段更像问题发生处）

---

# 19. v0.03a 完成后的系统形态（更明确）

gewyvern v0.03a 是：

> Linux TCP 行为调试器 runtime（模板化切面 + 窗口化拼装 + 段落定位 + 可验证因果链）

不是：

* 观测平台
* 流量分析器
* 抓包工具

---

# 20. v0.04 之后才做的事（保持你的列表，补一条）

* break/watch
* queue engine（真实）
* redirect/replay
* UDP
* multi-host
* **自定义模板 DSL**

---
