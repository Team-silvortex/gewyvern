# 🐉 gewyvern v0.03b — Handshake Template Runtime (Ringbuf Skeleton)

**Status:** Draft (0.03b)
**Scope:** TCP-only
**Nature:** Single-host / CLI-first / session runtime
**Orientation:** **network debugger（不是 observability）**
**Runtime Stack:** **eBPF C (CO-RE) + Rust runtime + ringbuf**

---

## 0. v0.03b 的版本定位

**0.03a 是 runtime contract。0.03b 是第一次“让它跑起来并可验证”的 skeleton。**

0.03b 的唯一任务：

> **把 T1(handshake_debug) 从 attach → ingest → ledger → flow → reason(L0/L1) → CLI → export 跑通，并能在真实场景中复现与验收。**

---

## 1. 0.03b 的新增约束（比 0.03a 更“硬”）

0.03b 只允许新增“闭环必需”的工程结构，不允许扩展能力边界：

1. **只实现一个模板：T1 handshake_debug（T2/T3 仅允许 stub，不验收）**
2. **只实现 L0→L1（不实现 L3 narrative）**
3. **Intervention 默认禁用（drop SYN/RST 不可用）**
4. **事实通道固定为 ringbuf（不做 perfbuf）**
5. **强制 Export-first：每个可验收场景必须能导出 JSON 且可追溯 FactId**

---

## 2. 0.03b 不做什么（必须锁死）

* ❌ 不开放自定义模板 / DSL
* ❌ 不支持 UDP / multi-proto
* ❌ 不支持 multi-host
* ❌ 不做 GUI
* ❌ 不做 queue engine / payload / seq-order reconstruction
* ❌ 不做 ML / AI 推理
* ❌ 不做 L3 业务解释
* ❌ 不引入长期观测与持久化平台化能力（window-bounded 不变）

---

## 3. 0.03b 的唯一目标（可验收闭环）

0.03b 必须同时成立：

1️⃣ **template attach 成功（handshake_debug）**
2️⃣ **fact stream 进入 ledger（append-only）**
3️⃣ **flow emergent 成功（握手连续体可拼装）**
4️⃣ **reason L0→L1 可生成（含 breakpoints + segments）**
5️⃣ **CLI 可 inspect / reason show**
6️⃣ **export json 可读、可追溯（含 schema/version/coverage）**

---

## 4. Runtime 架构（0.03b 不改层次，只补“闭环组件”）

```
kernel plane (eBPF C, CO-RE)
    ↓ ringbuf
Fact Stream
    ↓
Ledger (append-only)
    ↓
Flow Registry (projection → FlowSnapshot)
    ↓
Reason Engine (L0→L1, lazy)
    ↓
CLI Debugger
    ↓
Export (JSON)
```

0.03b 新增（闭环必需）：

* **Attach Coverage Engine**
* **Window Watermark**
* **Schema/Versioning**
* **Identity Confidence**

---

## 5. Template Runtime（0.03b 仅实现 T1）

### 5.1 T1: handshake_debug（唯一可运行模板）

**目的：** 三次握手失败“段落定位”（segment-level）
**窗口：** `W = 5s`（固定）
**lateness：** `200ms`（固定，watermark 用）

**必须采集 FactKind：**

* `TCP_STATE_TRANSITION`
* `TCP_PACKET_META`（仅 SYN/SYN-ACK/ACK 的 header meta 与方向信息）
* `ROUTE_META`（简化 fingerprint）

**允许 Filters（仅影响采集，不影响 identity）：**

* `tcp_key`（src/dst ip:port + direction）
* `netns`
* `iface`（ifindex）
* `pid/cgroup`（best-effort，仅展示，不进入 identity）

> 强规则：**没有模板 = 不允许 attach。**
> 强规则：**没有 window = 不允许 session start（默认 5s + 200ms lateness）。**

---

## 6. Facts（事实层）— 0.03b 固化最小集

Ledger 仍然“只记录事实，不解释”。

### 6.1 FactKind（0.03b）

**必须：**

* `TCP_STATE_TRANSITION`
* `TCP_PACKET_META`
* `ROUTE_META`

**用户态事实：**

* `SESSION_MARK`（start/stop）
* `ATTACH_REPORT`（见 7）
* `WINDOW_MARK`（open/freeze/close）

### 6.2 每条事实最小字段

所有 Fact 必须携带：

* `ts`（kernel timestamp）
* `cpu`
* `session_id`
* `netns`
* `ifindex`
* `fact_kind`
* `fact_id`（用户态分配，append-only 序号或 UUID）

---

## 7. Attach Coverage Engine（0.03b 必须新增）

目的：让 reason 能区分：

* 事件真的没发生
  vs
* 事件发生了但没抓到（attach/丢包/溢出）

### 7.1 AttachReport（导出必须包含）

至少包含：

* `required_fact_kinds`：seen/missing（对 T1 的 3 类）
* `hookpoints`：attached/failed（列出实际 attach 点）
* `ringbuf_lost_events`：计数（若可取）
* `probe_build_id`：git sha / build hash

> 0.03b 验收要求：当输出“missing step”时，必须同时输出 attach coverage 以支撑可验证性。

---

## 8. Window（0.03b 加 watermark）

窗口不是性能优化，是 correctness 约束。

### 8.1 固定窗口参数

* `duration = 5s`
* `lateness = 200ms`
* `freeze_at = end_ts + lateness`（watermark）

### 8.2 规则

* **flow 拼装只在 window 内进行**
* watermark 之后 **freeze snapshot**
* freeze 后不再接纳该 window 的事实进入 flow view（late events 仅计入 dropped/late stats）

---

## 9. Flow Registry（0.03b 的核心产物：FlowSnapshot）

0.03b 的“运行时输出本体”是 **FlowSnapshot**。
CLI/Export 都围绕 snapshot 工作。

### 9.1 Flow identity（TCP-only，保持 0.03a 逻辑）

flow :=

* TCP lifecycle continuity
* path continuity
* time continuity (within window)

不依赖 PID/socket fd/iface。

### 9.2 Identity 匹配（0.03b 保留优先级 + window 约束）

1. `(netns, sk_cookie)` 且在同 window 内
2. `(netns, TcpKey + time continuity)` 同 window 内连续
3. `TcpKey + packet 行为连续（握手片段连续）` 同 window 内

### 9.3 Identity Confidence（0.03b 新增）

每个 flow 必须输出：

* `sk_cookie_match: bool`
* `tcp_key_match: bool`
* `time_continuity: bool`
* `score ∈ [0,1]`

> 无 ML，只做可解释打分。

---

## 10. Segment（段落定位）— 0.03b 最小可用版本

0.03b 段落目标：

> 给出“握手断裂更像发生在网络路径/栈阶段的哪一段”，不是精确定位队列/路由内部细节。

最小 segment 字段：

* `segment_id`
* `route_fingerprint`（简化，需稳定字段）
* `ifindex/oif`（若可得）
* `start_ts/end_ts`
* `evidence_refs`（导致段落划分或变化的 FactId）

0.03b 允许只生成 1 个 segment（无变化场景）。

---

## 11. Reason Engine（0.03b 只做 L0→L1）

### 11.1 L0（事实）

* packet/state/route

### 11.2 L1（结构化输出，0.03b 必须）

必须包含：

* `timeline`（握手关键事件序列）
* `handshake_phase`（SYN seen / SYN-ACK seen / ACK seen）
* `breakpoints`（断裂点：哪一步缺失或异常）
* `segments`（至少 1 段）
* `evidence_refs`（可追溯 FactId）
* `attach_coverage_ref`（指向 AttachReport）

> 0.03b 不输出 L3，不做叙事，不做 AI 推理。

### 11.3 Breakpoints（0.03b 最小集合）

* `missing_syn`
* `missing_synack`
* `missing_ack`
* `abnormal_state_transition`（可选）
* `possible_not_captured`（当 coverage 不完整时）

---

## 12. CLI Contract（0.03b 固定最小命令集）

必须实现：

* `session start --template handshake_debug`
* `session stop`
* `flow list`
* `flow inspect <flow_id>`（输出 FlowSnapshot）
* `reason show <flow_id>`（输出 L1）
* `export json --session <id> [--flow <id>]`

允许存在但不验收：

* `attach tcp`（幂等，通常由 session start 隐式触发）

---

## 13. Export-first（0.03b 强制）

### 13.1 Export JSON 必须包含

* `schema_version`
* `template_id` + `template_version`
* `probe_build_id`
* `window_params`（duration, lateness, freeze_ts）
* `attach_report`
* `ledger_stats`（fact count, late count, lost count）
* `flow_snapshot`（含 segments + identity_confidence）
* `reason_l1`（含 breakpoints + refs）
* `session_marks`

目标：**一 json 多吃**

* CLI debug
* leserpent 元数据（未来 protobuf）
* 外部工具/LLM 读取解释（但 0.03b 不在 runtime 内做 L3）

---

## 14. eBPF 实现约束（0.03b：C + CO-RE + ringbuf）

* eBPF 端只负责产出结构化事件（Facts），不做解释
* 使用 ringbuf 输出
* 事件结构必须包含：ts/cpu/netns/ifindex/kind + payload（按 kind）

**最小 hook 集只需覆盖 T1：**

* 能产出握手 SYN/SYN-ACK/ACK 的 `TCP_PACKET_META`
* 能产出 `TCP_STATE_TRANSITION`
* 能产出 `ROUTE_META`（简化 fingerprint）

> 0.03b 不追求完美覆盖，追求“能跑 + 可验证 coverage”。

---

## 15. v0.03b 验收标准（只验收 3 场景）

所有场景必须满足：

* 生成 FlowSnapshot（含 segments）
* 生成 Reason L1（含 breakpoints）
* evidence 可追溯（FactId refs）
* export json 可导出且包含 coverage/version/window

### 场景 1：正常三次握手

* phases 完整（SYN/SYN-ACK/ACK）
* breakpoints 为空
* segments = 1（稳定）
* confidence 高（基于 identity_confidence + 覆盖）

### 场景 2：SYN-ACK 缺失（例如防火墙丢）

* breakpoint = `missing_synack`
* attach coverage 完整（证明不是没抓到）
* segments 至少 1

### 场景 3：路径段落变化（route fingerprint 改变）

* segments ≥ 2（或 diverge evidence 出现）
* reason L1 指出“疑似 segment 变化发生点”
* evidence refs 可追溯

---

## 16. 0.03b 完成后的系统形态

gewyvern v0.03b 是：

> **Linux TCP handshake 调试器 runtime（模板化切面 + 窗口化拼装 + 段落定位 + 可验证 L1 因果结构 + export-first）**

不是：

* 观测平台
* 抓包工具
* 流量分析器

---

## 17. v0.04 才做的事（延后清单）

* 自定义模板 / DSL
* T2/T3 的完整实现与验收
* L3 narrative（可选，且必须可追溯）
* retrans/jitter 的更强证据引擎
* intervention（drop SYN/RST）
* queue engine（真实）
* UDP / multi-proto
* multi-host

