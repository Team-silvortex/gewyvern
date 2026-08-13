# Runtime Window Workspace

Leserpent 的 `Child Panel` 是面向多个 gewyvern runtime 的窗口工作台。它为每个已打开的 runtime 保留独立窗口和视图上下文，但只让当前活动窗口持有远端嵌入内容。

## Operator Workflow

打开 runtime 子窗口有三种方式：

1. 在 runtime 列表中选择 `Open Panel`。
2. 进入 `Child Panel` 后选择 `Open Selected`。
3. 使用 `Open All` 按当前 fleet 顺序填充工作区，最多保留 8 个 runtime 窗口。

窗口标题栏提供：

- 激活窗口
- 当前 runtime 可信状态
- 在新标签页中打开当前视图
- 关闭单个窗口

工作台工具栏还提供 `Close All`。关闭窗口会卸载对应 iframe 并更新浏览器本地窗口偏好，但不会删除 runtime，也不会修改 Orchestra 或控制面 JSON/SQLite 持久化状态。

## Active Window Semantics

顶部的 source 和 view 控件只操作当前激活窗口：

- runtime 与可选 sidecar 之间的 source 切换
- Home、Health、Latest Meta、Summary、Analysis 等视图切换
- `Open in New Tab`

每个窗口在 `runtimeWindowViews` 中保存自己的视图。激活另一个窗口时，Leserpent 会恢复该窗口上次使用的视图，不会改写其他窗口的视图选择；先前的活动窗口则进入暂停态并卸载远端内容。

只有活动窗口会加载远端 iframe。非活动窗口保留 runtime 名称、视图、目标与可信状态，但会卸载远端页面并显示暂停壳；再次激活时才恢复该窗口的 URL。这让“窗口上下文”和“远端连接生命周期”彼此独立。

窗口标题按钮支持 roving keyboard navigation：方向键在窗口间移动，`Home` 和 `End` 跳到首尾。关闭活动窗口时优先激活并聚焦其右侧相邻窗口；关闭末尾窗口时回到前一个窗口。关闭最后一个窗口后，焦点回到 `Open Selected`，因此键盘操作链不会掉回页面根节点。

## Persistence And Deep Links

窗口状态分成两层：

- URL 中的 `runtimeId`、`runtimePane=panel` 和 `runtimeView` 决定 deep-link 当前激活的 runtime 与视图。
- `localStorage` 中的 `leserpent.runtimeWindows` 恢复打开的窗口集合、激活窗口和每个窗口的视图。

URL 意图优先于本地恢复状态。`runtimeId` 会先成为待验证的活动窗口，dashboard 数据加载后再清理无效或已删除的 ID，避免干净浏览器中的 Child Panel deep-link 落入空工作区。

本地恢复状态会在使用前执行协议边界校验：JSON 最多读取 64 KiB、runtime ID 去重、单个 ID 最长 256 字符、视图必须属于已知集合、窗口最多 8 个，并使用无原型对象重建 view map。异常或旧版本数据不会直接进入 DOM 渲染链。

窗口工作台状态属于浏览器本地偏好，不进入 Leserpent JSON/SQLite 控制面持久化，也不会在不同操作员之间同步。

## Performance Boundaries

窗口渲染遵守以下约束：

- 使用 runtime ID 进行 keyed DOM reconciliation。
- 更新一个窗口时不重建其他窗口 DOM。
- 工作区硬上限为 8 个窗口；`Open All` 超出上限时给出明确反馈。
- 只有活动窗口加载远端 iframe，非活动窗口立即切回 `about:blank`。
- iframe 仍使用 `loading="lazy"`，并保持无权限 sandbox 与 `no-referrer`。
- 关闭或切换窗口时卸载对应远端 iframe。
- 离屏窗口使用 `content-visibility: auto` 降低布局和绘制成本。
- 桌面端在空间允许时双列显示，`920px` 以下强制单列。

因此大型 fleet 也不会因为一次 `Open All` 同时维持大量 iframe 或远端响应文档。需要查看第 9 个 runtime 时，操作者先关闭一个已有窗口，避免隐式淘汰正在使用的上下文。

## Failure And Trust States

当 runtime 未观测、抓取失败、没有可用 snapshot，或者 sidecar 未配对时，窗口显示紧凑的操作型 blank state，而不是加载不可信或不存在的嵌入页面。

blank state 会保留：

- source 与 view
- runtime 目标地址
- 当前可信状态
- 建议的刷新动作

这些状态不会阻塞其他 runtime 窗口工作。

## Implementation Map

- `src/Leserpent/frontend/40-runtime-inspector.ts`
  - 窗口生命周期、持久化、keyed rendering 和 active-window 语义
- `src/Leserpent/frontend/30-runtime-panel-helpers.ts`
  - URL、capability、trust 与 blank-state 计算
- `src/Leserpent/frontend/15-preferences-bootstrap.ts`
  - 窗口和工具栏事件绑定
- `src/Leserpent/frontend/20-security-transport.ts`
  - deep-link hydration 与 URL 优先级
- `src/Leserpent/frontend/47-runtime-list-renderer.ts`
  - runtime 列表中的 `Open Panel` 入口
- `src/Leserpent/wwwroot/styles.css`
  - 双列/单列窗口布局和 anti-overlap 规则

修改后必须重新生成静态前端：

```bash
cd apps/leserpent
npm run check:frontend
npm run build:frontend
dotnet build src/Leserpent/Leserpent.csproj
```

## Validation Checklist

- 两个 runtime 可以同时显示，且只有一个 active window。
- 只有 active window 的 iframe 持有远端 `src`，切换后旧窗口回到暂停壳。
- `Open All` 在 8 个窗口处停止，并对 fleet 剩余数量给出反馈。
- 在窗口 A 切换视图后激活窗口 B，A 的视图保持不变。
- 刷新页面后窗口集合和各自视图恢复。
- 带 `runtimeId` 和 `runtimeView` 的 deep-link 覆盖本地 active window。
- 污染、重复、超长或未知 view 的本地恢复数据会被安全归一化。
- 关闭 active window 后安全切换并聚焦剩余窗口；关闭最后一个窗口后聚焦 `Open Selected`。
- 方向键、`Home` 和 `End` 可以切换窗口且焦点保持在活动标题。
- 删除 runtime 后不留下失效窗口。
- 桌面双列没有控件碰撞，移动单列没有横向溢出。
- 浏览器控制台没有 warning 或 error。
