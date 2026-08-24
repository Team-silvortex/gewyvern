# Leserpent Language Packs

Leserpent 支持在不重新构建控制面的情况下安装附加 UI 语言。内置语言仍随应用编译发布；附加语言包从同源 catalog 安装，或由操作员从本地 JSON 文件导入。

当前官方维护总数为 30：

- 8 个内置完整 catalog：English、简体中文、繁體中文、日本語、Español、Deutsch、Français、한국어
- 22 个可下载 `core-ui` pack：Português (Brasil)、Italiano、Русский、العربية、हिन्दी、বাংলা、Bahasa Indonesia、Bahasa Melayu、ไทย、Tiếng Việt、Türkçe、Polski、Nederlands、Українська、Čeština、Svenska、Dansk、Norsk、Suomi、Ελληνικά、עברית、فارسی

`core-ui` 表示主 shell、语言包中心、主题、顶层导航和 runtime 子窗口入口由官方维护。当前官方 `1.1.0` 产物拥有严格一致的 30 个键；Web 与 Desktop 仍接受旧的 18 键兼容基线，因此升级不会让既有 `1.0.0` 包失效。复杂诊断、协议细节和 Orchestra 长尾文案暂时回退 English。阿拉伯语、希伯来语和波斯语会启用 RTL document direction。

## Web Workflow

在 dashboard 顶部打开 `Language Packs`：

- `Refresh Catalog`：读取 `/language-packs/catalog.json`
- `Install`：下载、校验并安装 catalog 中的语言包
- `Download`：下载经过校验的原始语言包 JSON，但不安装
- `Import JSON`：安装操作员本地选择的 JSON 文件
- `Export`：导出已安装语言包
- `Remove`：从当前浏览器卸载附加语言包

安装完成后，新 locale 会出现在语言选择器中。卸载当前正在使用的语言包时，UI 自动回退到 `Follow Browser`。

Web 语言包仅保存在当前浏览器的 `localStorage`，不会上传到 Leserpent、写入控制面 JSON/SQLite，或同步给其他操作员。

## Native Desktop Workflow

在 Avalonia Hub 或 macOS application menu 打开 `Language...`：

- 选择 22 个可下载 locale 中的一个。
- 在 `Daemon catalog source` 中显式选择本机 Orchestra 或一个已保存的远程 daemon。
- 选择 `Download selected`，从该 daemon 的同源 `/language-packs/catalog.json` 下载并安装。
- 无网络时仍可用 `Install JSON...` 导入本地文件，也可随时 `Remove pack`。

Desktop 下载只复用连接配置中的 HTTPS origin 与已保存 CA，不解析或读取该 daemon 的管理 token，也不会发送 bearer 或 `X-Leserpent-Admin-Token`。Rust `leserpentd` 直接内嵌并提供同一份 catalog 与 22 个 pack；这些公开 GET 路由会反向拒绝任何 bearer/admin header，因此成功下载同时证明客户端没有把控制面凭证带入公开内容域。托管 Web host 对同一路径执行相同拒绝策略。catalog 限制为 128 KiB，pack 限制为 256 KiB；禁止重定向、跨源 URL、catalog 外 locale，以及 digest、locale、version 不一致。窗口关闭会取消请求，同一窗口一次只允许一个语言包操作。

Desktop 将通过校验的 pack 原子写入当前用户的私有 `language-packs-v1` 目录。它不写入控制面状态，也不会在 daemon 间同步。catalog 下载拥有来源 CA、同源路径和 SHA-256 绑定，并在创建目录或替换文件前要求当前官方 `1.1.0` 精确 30 键契约；被该官方契约拒绝的首次安装不产生目录，被拒绝的升级保留原文件且不遗留临时文件。本地 JSON 导入则保留 18 键兼容基线，只拥有结构与资源边界校验，不宣称 catalog 身份。

## Pack Format

语言包使用 `leserpent.language-pack/v1`：

```json
{
  "schema": "leserpent.language-pack/v1",
  "locale": "pt-BR",
  "name": "Portuguese (Brazil)",
  "nativeName": "Português (Brasil)",
  "version": "1.1.0",
  "author": "Leserpent community",
  "direction": "ltr",
  "coverage": "core-ui",
  "translations": {
    "hero": {
      "title": "Painel do plano de controle"
    }
  }
}
```

`locale` 使用 BCP 47 风格标签。语言包允许只包含部分翻译；缺失键会回退到内置 English catalog。兼容输入至少覆盖 18 个稳定 shell 键，官方 `1.1.0` 发布包则必须精确覆盖 30 个已声明键。

语言包不能覆盖内置 locale：

- `en`
- `zh-CN`
- `zh-TW`
- `ja`
- `es`
- `de`
- `fr`
- `ko`

## Validation And Security

安装前会执行：

- schema 与 metadata 校验
- locale 格式校验
- 最大 256 KiB 文件限制
- 最大 12 个附加语言包
- 浏览器总存储 payload 最大 512 KiB
- translation tree 深度、节点数和字符串长度限制
- 拒绝数组、非字符串叶节点、控制字符和危险对象键
- catalog URL 必须同源且位于 `/language-packs/`
- catalog 安装必须通过 SHA-256
- 下载包 locale/version 必须匹配 catalog 条目
- Native Desktop catalog 安装必须在写盘前匹配当前官方 version 与精确 key set
- Native Desktop 官方契约拒绝不得创建新状态或覆盖既有 pack
- Native Desktop catalog 必须完整覆盖固定的 8 built-in + 22 downloadable roster
- Native Desktop 下载仅使用显式选择的 daemon origin 与 CA，且不发送管理凭证
- Rust daemon 与托管 Web host 都拒绝公开语言包请求携带 `Authorization` 或 `X-Leserpent-Admin-Token`

Leserpent 不接受任意远程语言包 URL，也不会由服务端代替浏览器抓取第三方地址。这样可以避免 SSRF，并把远程供应链边界限制在随 Leserpent 发布的同源静态 catalog。

本地导入包不会拥有 catalog 信任标记，但仍必须通过所有结构和资源限制校验。

## Publishing A Pack

官方包的 source of truth 位于 `scripts/build-language-packs.mjs`。添加或修改定义后运行：

```bash
npm run build:language-packs
```

发布前必须再通过 Rust 原生前端 packager 刷新并复核内容寻址 manifest：

```bash
npm run package:frontend
npm run verify:frontend-package
```

生成器会清理旧 JSON、重建 22 个官方 pack、计算 SHA-256，并原子生成 catalog metadata。生成后：

1. 检查 locale、native name、direction 和翻译质量。
2. 运行前端与 .NET 构建，并确认 `frontend-package-manifest.json` 绑定当前 pack 大小与摘要。
3. 运行 `LanguagePackArtifactTests`，确认每个官方包都是 `1.1.0` 且精确覆盖 30 键。
4. 从实际发布服务下载文件并再次核对 digest。

catalog 使用：

```json
{
  "schema": "leserpent.language-pack-catalog/v1",
  "packs": [
    {
      "locale": "pt-BR",
      "nativeName": "Português (Brasil)",
      "version": "1.1.0",
      "direction": "ltr",
      "coverage": "core-ui",
      "url": "/language-packs/pt-BR.json",
      "sha256": "..."
    }
  ]
}
```

任何语言包文件变化都会改变 digest，必须同步更新 catalog。

## Implementation Map

- `src/Leserpent/frontend/19-language-packs.ts`
  - 校验、catalog、SHA-256、安装、导入、导出和卸载
- `src/Leserpent/frontend/09-i18n-base.ts`
  - translation merge 与 English fallback 基础
- `src/Leserpent/frontend/15-preferences-bootstrap.ts`
  - Language Packs UI 事件
- `src/Leserpent/frontend/20-security-transport.ts`
  - 浏览器语言解析和动态 locale 应用
- `src/Leserpent/wwwroot/language-packs/`
  - 同源 catalog 与官方发布包
- `apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopLanguagePackCatalogClient.cs`
  - Native Desktop 的无凭证 CA-bound catalog 下载、严格解码和 SHA-256 校验
- `apps/leserpent-avalonia/src/Leserpent.Avalonia/SavedDaemonLanguagePackVerifier.cs`
  - 持久化连接 catalog、唯一受管 CA、错误 CA 拒绝和输入不变的发布验证
- `apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopLanguagePackStore.cs`
  - Native Desktop 的手动/官方安装信任分层、写盘前官方契约校验与私有原子存储
- `crates/leserpentd/src/language_packs.rs` 与 `crates/leserpentd/src/remote.rs`
  - Rust daemon 的编译期资产白名单、公开 GET 路由和凭证域隔离
- `src/Leserpent/ControlPlane/LanguagePackRequestPolicy.cs`
  - 托管 Web host 对公开语言包路径执行相同的凭证拒绝策略
- `scripts/build-language-packs.mjs`
  - 30-locale roster 中 22 个下载包的翻译源、pack 生成与 digest catalog

建议在每次打包前先执行 `npm run verify:language-packs`，用于快速核对内置 locale 对齐和 22 个官方下载包的核心覆盖率。

Native Desktop 发布前再让实际桌面解码器验证同一批产物：

```bash
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-desktop-language-pack-artifacts \
  "$PWD/apps/leserpent/src/Leserpent/wwwroot/language-packs/catalog.json" \
  "$PWD/apps/leserpent/src/Leserpent/wwwroot/language-packs"
```

该命令逐包执行 catalog SHA-256、locale/version、18 键兼容基线、写盘前官方 `1.1.0` 精确 30 键契约和 Desktop 私有存储 roundtrip，不发起网络请求。

本地 Orchestra 的真实 TLS 路径可用同一个桌面二进制验证：

```bash
cargo build -p leserpentd --features native-ssh
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-local-orchestra target/debug/leserpentd
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-saved-daemon-language-pack target/debug/leserpentd
```

该验证启动真实 Rust daemon，经私有 CA 下载 `pt-BR` catalog entry 与 pack，完成 digest/locale/version 绑定、私有安装、读取和清理。daemon 会拒绝带认证头的公开请求，因此成功往返也是无 bearer/admin header 的端到端证明。

第二个入口把同一在线 authority 写入生产连接 catalog，经独立受管 CA store
重新加载并通过 `DesktopLanguagePackSource.FromConnection` 下载。它先要求一个
错误 CA 在 TLS 阶段失败，再要求唯一选中 CA 成功，并验证 catalog、CA 摘要及
私有语言包仓库在清理后保持预期状态。两条真实路径都要求下载到当前
`1.1.0`/30 键官方产物，而不是只满足旧的 18 键最低基线。

2026-08-24 的 macOS arm64 发布验证进一步从 ad-hoc 签名的 `1.16.0` NativeAOT `.app` 内运行客户端与 daemon；机器可读证据保存在 `docs/fixtures/leserpent_language_pack_local_orchestra_native_aot_macos_arm64_20260824.json`。同日的 physical Linux x86_64 验证通过远程门禁执行锁定 NativeAOT 构建和两套真实往返，本地再严格复核文件清单、ELF/资产摘要、Local Orchestra 20 项断言、saved-daemon 12 项断言与凭证缺失；证据保存在 `docs/fixtures/leserpent_language_pack_local_orchestra_native_aot_linux_x86_64_20260824.json`。两平台的保存连接证明均已完成；当前剩余项是六个内置候选 catalog、新增 12 键下载包文案的母语审阅，以及 30 键之外的长尾扩展。

## Validation Checklist

- catalog digest 与服务器实际返回文件一致。
- 安装后 locale 立即出现在语言选择器中。
- 切换语言后静态 shell 和动态 dashboard 都重新渲染。
- 刷新页面后已安装包和语言偏好恢复。
- 卸载当前语言时自动回退，不留下无效 `<option>`。
- 错误 schema、超限文件和危险 translation key 被拒绝。
- catalog digest、locale 或 version 不匹配时安装失败。
- 窄屏和暗色主题下弹层无重叠、文字可读。
- 官方 roster 始终满足 8 built-in + 22 downloadable = 30 locales。
- 22 个官方下载包保持 `1.1.0`、精确 30 键，旧的 18 键兼容输入仍可安装。
- RTL pack 安装后 `document.dir=rtl`，卸载或切换 LTR 语言后恢复。
