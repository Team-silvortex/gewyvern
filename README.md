---

# 🐉 gewyvern v0.03 — Integrated Design

### TCP Network Debugger Runtime (Engineering-first)

Status: Draft
Scope: TCP-only
Nature: Single-host / CLI-first / session runtime
Orientation: **network debugger（不是 observability）**

---

# 0. 核心定位（必须统一）

gewyvern 是：

> **protocol-behavior debugger runtime**

不是：

* 抓包工具
* observability 平台
* tracing 系统
* 网络策略引擎

它做的是：

> 用 eBPF 捕获“物理事实”，在用户态构建“行为连续性”，并输出“可验证因果链”。

---

# 1. v0.03 的唯一目标

让 gewyvern 第一次成为：

> **可以像调试器一样工作的系统**

必须同时成立：

1️⃣ attach 成功
2️⃣ flow emergent 成功
3️⃣ reason chain 可生成
4️⃣ intervention 能被 gate 控制并执行

---

# 2. 用户问题范围（严格限制）

v0.03 只解决三类问题：

## 2.1 连接为什么失败

* SYN 是否发出
* SYN-ACK 是否到达
* ACK 是否送出
* 哪个阶段断裂

## 2.2 为什么卡顿 / 抖动

* 重传迹象
* 路由变更迹象
* 队列症状迹象（不做完整 queue engine）

## 2.3 为什么被断开

* FIN / RST
* 断开前状态演化
* 路径变化证据

---

# 3. Runtime 架构（最终形态）

```
kernel plane (eBPF)
    ↓
Fact Stream
    ↓
Ledger (append-only)
    ↓
Flow Registry (projection)
    ↓
Reason Engine
    ↓
Gate
    ↓
CLI Debugger
```

---

# 4. 设计原则（不可改变）

### 4.1 Evidence-first

所有输出必须能追溯到物理事实。

### 4.2 Flow emergent

flow 不由用户定义。

### 4.3 Session boundary

session 是观测沙箱。

### 4.4 Lazy interpretation

reason 不实时构建，按需生成。

### 4.5 Observe fully or refuse

scope 不完整 → 禁止干预。

---

# 5. 执行入口（protocol anchored）

v0.03 attach 只允许 3 类事实：

### 必须

* TCP state transition
* TCP packet meta
* route decision meta

### 可选

* socket lineage

---

# 6. Ledger（事实层）

唯一职责：

> 记录事实，不解释。

特性：

* append-only
* 可回放
* 事实不可改
* 每条带：

  * ts
  * cpu
  * netns
  * ifindex
  * session
  * fact_kind

---

# 7. Flow Registry（runtime 核心）

Flow Registry = gewyvern 的大脑。

### 两层结构：

## 7.1 Flow Ledger（事实引用层）

只存 FactId。

## 7.2 Flow View（可变解释层）

包含：

* lifecycle
* path segments
* evidence index
* anchor
* state snapshot

---

# 8. Flow 定义（TCP-only）

```
flow :=
  TCP lifecycle continuity
  + path continuity
  + time continuity
```

不依赖：

* PID
* socket fd
* interface

---

# 9. Identity 匹配规则

优先级：

1. (netns, sk_cookie)
2. (netns, TcpKey + 时间连续)
3. TcpKey + packet 行为连续

---

# 10. Diverge（硬规则）

### path change = new flow

条件：

* oif/gw 改变
* 持续时间超过 T_diverge_min
* 或 evidence 次数 > N

结果：

* 新 FlowId
* 旧 flow 标记 diverged
* anchor 延续

---

# 11. Reason Chain（只允许 L0→L1→L3）

## L0

* packet
* state
* route

## L1

* timeline
* lifecycle
* path segments

## L3

* narrative（可追溯）

不允许：

* 用户注释生成
* AI推理
* 业务解释

---

# 12. Intervention Model

## 决策单位

flow

## 执行单位

packet

## v0.03 只支持：

* drop SYN
* drop RST

---

# 13. Safety Gate

必须满足：

1. session scope 存在
2. attach 完整闭环
3. reason chain 足够
4. CLI confirm

否则拒绝。

---

# 14. Lazy Interfere

规则：

* session 内禁止 sampling
* incomplete observation 禁止 attach

---

# 15. CLI Contract

v0.03 命令集固定：

* session start
* session stop
* attach tcp
* flow list
* flow inspect
* reason show
* drop syn/rst
* export json

---

# 16. 数据输出

输出类型：

* event stream
* flow snapshot
* reason chain
* intervention log

格式：

* protobuf native
* JSON export

---

# 17. 实现模块（最终划分）

```
gewyvern-core
  ledger
  registry
  reason
  gate
  snapshot

gewyvern-ebpf
  probes
  maps

gewyvern-loader
  attach
  normalize

gewyvern-cli
  session
  commands
```

---

# 18. ingest_fact 核心机制

```
append ledger
→ match identity
→ emerge / merge
→ maybe diverge
→ update lifecycle
→ mark reason dirty
```

---

# 19. Reason 生成（lazy）

只在：

* flow inspect
* reason show

时生成。

---

# 20. v0.03 验收标准（最重要）

必须跑通 6 场景：

1. 正常三次握手
2. SYN 无 SYN-ACK
3. SYN-ACK 无 ACK
4. RST 断开
5. 丢包 → retrans迹象
6. 路由变更 → diverge

全部必须：

* 生成 reason chain
* 可追溯事实
* CLI 可展示

---

# 21. v0.03 完成后的系统形态

不是：

* 观测平台
* 网络分析器

而是：

> Linux TCP 行为调试器。

---

# 22. v0.04 才会进入的领域

* break/watch
* queue engine
* redirect/replay
* UDP
* multi-host

---

# 23. 最关键的“设计完成标志”

不是写完代码。

而是：

> 当用户可以用 gewyvern 像 gdb 一样“盯着一个 flow 看它发生什么”。

那一刻，系统成立。

---
