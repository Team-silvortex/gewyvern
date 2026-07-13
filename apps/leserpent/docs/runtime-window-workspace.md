# Runtime Window Workspace

Leserpent 的 `Child Panel` 是面向多个 gewyvern runtime 的窗口工作台。它不是把所有实例轮流塞进同一个 iframe，而是为每个已打开的 runtime 保留独立窗口、视图和嵌入内容。

## Operator Workflow

打开 runtime 子窗口有三种方式：

1. 在 runtime 列表中选择 `Open Panel`。
2. 进入 `Child Panel` 后选择 `Open Selected`。
3. 使用 `Open All` 打开当前控制面已加载的全部 runtime。

窗口标题栏提供：

- 激活窗口
- 当前 runtime 可信状态
- 在新标签页中打开当前视图
- 关闭单个窗口

工作台工具栏还提供 `Close All`。关闭窗口只卸载对应 iframe，不删除 runtime，也不修改 Orchestra 或持久化状态。

## Active Window Semantics

顶部的 source 和 view 控件只操作当前激活窗口：

- runtime 与可选 sidecar 之间的 source 切换
- Home、Health、Latest Meta、Summary、Analysis 等视图切换
- `Open in New Tab`

每个窗口在 `runtimeWindowViews` 中保存自己的视图。激活另一个窗口时，Leserpent 会恢复该窗口上次使用的视图，不会改写或重载其他窗口。

## Persistence And Deep Links

窗口状态分成两层：

- URL 中的 `runtimeId`、`runtimePane=panel` 和 `runtimeView` 决定 deep-link 当前激活的 runtime 与视图。
- `localStorage` 中的 `leserpent.runtimeWindows` 恢复打开的窗口集合、激活窗口和每个窗口的视图。

URL 意图优先于本地恢复状态。无效或已删除的 runtime ID 会在 dashboard 数据加载后被清理。

窗口工作台状态属于浏览器本地偏好，不进入 Leserpent JSON/SQLite 控制面持久化，也不会在不同操作员之间同步。

## Performance Boundaries

窗口渲染遵守以下约束：

- 使用 runtime ID 进行 keyed DOM reconciliation。
- 更新一个窗口时不重建其他窗口 DOM。
- iframe 使用 `loading="lazy"`。
- 关闭窗口时卸载对应 iframe。
- 离屏窗口使用 `content-visibility: auto` 降低布局和绘制成本。
- 桌面端在空间允许时双列显示，`920px` 以下强制单列。

`Open All` 会为当前 fleet 中的每个 runtime 建立窗口。大型 fleet 应优先按需打开，避免浏览器同时维持过多 iframe 和远程响应文档。

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
- 在窗口 A 切换视图后激活窗口 B，A 的视图保持不变。
- 刷新页面后窗口集合和各自视图恢复。
- 带 `runtimeId` 和 `runtimeView` 的 deep-link 覆盖本地 active window。
- 关闭 active window 后安全切换到剩余窗口。
- 删除 runtime 后不留下失效窗口。
- 桌面双列没有控件碰撞，移动单列没有横向溢出。
- 浏览器控制台没有 warning 或 error。
