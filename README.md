🐉 gewyvern v0.04 — Fragment IR Emergence Runtime

## Documentation

- [Project Overview](docs/overview.md)
- [Runtime Architecture](docs/architecture.md)
- [Development Guide](docs/development.md)
- [Headless Linux Guide](docs/headless-linux.md)

## TDD First

这个仓库现在以 TDD 方式推进。

默认开发入口：

- `cargo tdd`：先跑主行为规格
- `cargo tdd-one <test_name>`：只迭代一个场景
- `cargo tdd-rules`：跑规则规格
- `cargo test`：收口前跑全量

当前规格分为两层：

- `tests/runtime_tdd.rs`：场景/验收规格
- `tests/template_rules_tdd.rs` 与 `tests/fragment_rules_tdd.rs`：规则规格

Status: Draft (0.04)
Scope: TCP + UDP session debugging（仍不改变 debugger 本质）
Nature: Single-host / CLI-first / window-bounded session runtime
Orientation: network debugger（非 observability 平台）
Runtime Stack: eBPF C (CO-RE) + Rust runtime + ringbuf

⸻

0. 版本哲学

v0.03b 解决了闭环。

v0.04 的目标不是扩展能力，而是：

让 eBPF 代码从“文件集合”演进为“可组合片段集合”。

同时确立：

IR 从片段中生长
DSL 从 IR 中生长
不反向设计

⸻

1. v0.04 核心原则

1.1 Debugger First 不变

ge 仍然是：
	•	单机会话
	•	有窗口
	•	可 freeze
	•	可导出
	•	可复核

不允许演化为：
	•	长期观测平台
	•	持久化监控系统
	•	分布式 agent 编排

⸻

1.2 IR 不是编译目标

IR 的目标：

管理片段组合秩序

不是：

生成 eBPF 程序

eBPF 程序仍然手写 C。

⸻

1.3 模板 = 片段集合

模板不再直接对应 attach 逻辑。

模板现在等价于：

Template = Fragment Set + Window Profile + Reason Profile


⸻

2. Fragment（片段）— v0.04 新核心结构

2.1 定义

Fragment 是：
	•	一份 eBPF 程序（或其中一部分逻辑）
	•	一份 Rust 侧的描述文件（manifest）

每个 Fragment 必须具有唯一 ID。

⸻

2.2 Fragment Manifest（IR 胚胎）

struct FragmentDescriptor {
    id: &'static str,
    version: u32,

    hookpoints: Vec<HookPoint>,
    emits: Vec<FactKind>,
    requires: Vec<FactKind>,

    maps: Vec<MapSpec>,

    capabilities: Vec<CapabilityFlag>,
}

字段解释
	•	hookpoints：声明 attach 点
	•	emits：该片段产生的 FactKind
	•	requires：依赖哪些事实或前置片段
	•	maps：该片段使用的 BPF maps 规范
	•	capabilities：例如 tcp_state, packet_meta, route_meta

⸻

2.3 强规则
	•	Fragment 不允许解释事实
	•	Fragment 只产生结构化事实
	•	Fragment 不知道 window
	•	Fragment 不知道 reason

⸻

3. Fragment Registry

Runtime 内新增：

Fragment Registry

职责：
	•	注册所有可用 FragmentDescriptor
	•	校验 hookpoint 冲突
	•	校验 FactKind 冲突
	•	生成 Attach Plan

⸻

4. IR v0（隐式 IR）

v0.04 不实现完整 IR。

但 runtime 内部隐式存在：

4.1 IR 结构（只读）

HookGraph
FactGraph
DependencyGraph

HookGraph
	•	哪些 fragment attach 到哪些 hookpoint

FactGraph
	•	哪些 fragment 产出哪些 FactKind
	•	哪些 fragment 依赖哪些 FactKind

DependencyGraph
	•	片段间依赖关系
	•	是否允许并行

⸻

5. Template 重定义（v0.04）

5.1 handshake_debug（重构后）

T1 不再直接 attach。

定义为：

T1 =
    tcp_state_fragment
    tcp_packet_meta_fragment
    route_meta_fragment

window_profile = default_5s
reason_profile = handshake_l1

UDP 调试模板现在也已经存在：

UDP1 =
    udp_packet_meta_fragment
    route_meta_fragment

window_profile = default_5s
reason_profile = udp_datagram_l1


⸻

5.2 模板约束
	•	没有 Fragment Set → 不允许启动
	•	没有 window_profile → 不允许启动
	•	reason_profile 必须存在

⸻

6. Runtime 架构（更新）

Fragment Registry
    ↓
Attach Planner (IR based)
    ↓
kernel plane (eBPF fragments)
    ↓ ringbuf
Fact Stream
    ↓
Ledger
    ↓
Flow Registry
    ↓
Reason Engine
    ↓
CLI
    ↓
Export


⸻

7. Attach Planner（v0.04 新增）

根据 Fragment Set：
	1.	汇总 hookpoints
	2.	检查冲突
	3.	构建 attach plan
	4.	生成 AttachReport

AttachReport 现在包含：
	•	fragments_loaded
	•	hookpoints_attached
	•	hookpoints_failed
	•	required_fact_kinds_coverage
	•	ringbuf_stats

⸻

8. Flow / Window / Reason 不变（Debugger 本体不变）

8.1 Window
	•	duration = 5s（默认）
	•	lateness = 200ms
	•	freeze_at = end + lateness

冻结规则不变。

⸻

8.2 Identity / Confidence 不变

FlowSnapshot 结构保持 0.03b 版本。

新增：

fragment_sources: Vec<FragmentId>

用于追溯 flow 来自哪些片段。

⸻

9. Deterministic Replay（v0.04 强制）

新增规则：

L1 必须可由 Export JSON 重新计算

runtime 的 reason 只是在线计算。

Export JSON 必须包含：
	•	所有事实
	•	所有 fragment 描述
	•	window 参数
	•	coverage 报告

⸻

10. Export JSON（v0.04 扩展）

新增字段：

fragment_inventory: [
    { id, version }
]

attach_plan: {
    fragments,
    hookpoints,
    coverage
}


⸻

11. DSL 暂不实现，但预留边界

v0.04 不实现 DSL。

但明确未来 DSL 只能表达：
	•	fragment set 选择
	•	window 参数
	•	filters
	•	reason profile

DSL 不允许：
	•	自定义 FactKind
	•	自定义推理逻辑
	•	动态代码生成

⸻

12. v0.04 验收标准

仍然只验收 T1 三场景：
	1.	正常握手
	2.	SYN-ACK 缺失
	3.	route fingerprint 变化

但新增要求：
	•	attach_plan 必须可导出
	•	fragment_inventory 必须可导出
	•	L1 可 replay 重算一致

⸻

13. v0.04 完成后的系统形态

ge v0.04 是：

单机、窗口化、片段化、可验证、可重放的 TCP network debugger runtime

不是：
	•	观测平台
	•	分布式 agent
	•	协议解释引擎
	•	编译型 DSL 系统

⸻

14. 演进路线

v0.05
	•	新增 1–2 个协议片段
	•	扩展 fragment registry
	•	reason 仍然 L0→L1

v0.06
	•	IR v0 显式化
	•	attach planner 基于 IR graph

v0.07
	•	DSL v0（Template Assembly Language）
	•	仅做 fragment 组合声明

v1.0
	•	CLI niche debugger 稳定版
	•	再考虑 leserpent 多实例 orchestration

⸻

15. 最终哲学一句话

gewyvern 不是“解释网络”，
而是把一次网络异常压缩为可验证、可复核、可重放的证据链结构。
IR 与 DSL 只是降低扩展成本，而不是扩展解释权。

---

## TDD Workflow

从现在开始，gewyvern 采用测试驱动开发。

工作节奏固定为：

1. 先写或先扩展一个失败测试，表达一个行为、规则或回归场景
2. 再写最小实现，让测试变绿
3. 最后重构，但不能破坏 attach/export/replay 语义

当前测试分层：

- `tests/template_rules_tdd.rs`：模板约束测试
- `tests/fragment_rules_tdd.rs`：fragment registry / attach planner 规则测试
- `tests/runtime_tdd.rs`：面向 T1 场景的端到端行为测试

当前 T1 行为规格覆盖：

- 正常握手可导出 `attach_plan` 与 `fragment_inventory`
- `SYN-ACK` 缺失时，L1 replay 仍然一致
- route fingerprint 变化时，flow 必须切分
- freeze cutoff 之外的事实不能进入 export / replay

后续每增加一个 fragment、reason 规则、export 字段，都先补测试，再改实现。
