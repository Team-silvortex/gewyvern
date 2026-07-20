# Leserpent Language Packs

Leserpent 支持在不重新构建控制面的情况下安装附加 UI 语言。内置语言仍随应用编译发布；附加语言包从同源 catalog 安装，或由操作员从本地 JSON 文件导入。

当前官方维护总数为 30：

- 8 个内置完整 catalog：English、简体中文、繁體中文、日本語、Español、Deutsch、Français、한국어
- 22 个可下载 `core-ui` pack：Português (Brasil)、Italiano、Русский、العربية、हिन्दी、বাংলা、Bahasa Indonesia、Bahasa Melayu、ไทย、Tiếng Việt、Türkçe、Polski、Nederlands、Українська、Čeština、Svenska、Dansk、Norsk、Suomi、Ελληνικά、עברית、فارسی

`core-ui` 表示主 shell、语言包中心、主题、顶层导航和 runtime 子窗口入口由官方维护；复杂诊断、协议细节和 Orchestra 长尾文案暂时回退 English。阿拉伯语、希伯来语和波斯语会启用 RTL document direction。

## User Workflow

在 dashboard 顶部打开 `Language Packs`：

- `Refresh Catalog`：读取 `/language-packs/catalog.json`
- `Install`：下载、校验并安装 catalog 中的语言包
- `Download`：下载经过校验的原始语言包 JSON，但不安装
- `Import JSON`：安装操作员本地选择的 JSON 文件
- `Export`：导出已安装语言包
- `Remove`：从当前浏览器卸载附加语言包

安装完成后，新 locale 会出现在语言选择器中。卸载当前正在使用的语言包时，UI 自动回退到 `Follow Browser`。

语言包仅保存在当前浏览器的 `localStorage`，不会上传到 Leserpent、写入控制面 JSON/SQLite，或同步给其他操作员。

## Pack Format

语言包使用 `leserpent.language-pack/v1`：

```json
{
  "schema": "leserpent.language-pack/v1",
  "locale": "pt-BR",
  "name": "Portuguese (Brazil)",
  "nativeName": "Português (Brasil)",
  "version": "1.0.0",
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

`locale` 使用 BCP 47 风格标签。语言包允许只包含部分翻译；缺失键会回退到内置 English catalog。

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

Leserpent 不接受任意远程语言包 URL，也不会由服务端代替浏览器抓取第三方地址。这样可以避免 SSRF，并把远程供应链边界限制在随 Leserpent 发布的同源静态 catalog。

本地导入包不会拥有 catalog 信任标记，但仍必须通过所有结构和资源限制校验。

## Publishing A Pack

官方包的 source of truth 位于 `scripts/build-language-packs.mjs`。添加或修改定义后运行：

```bash
npm run build:language-packs
```

生成器会清理旧 JSON、重建 22 个官方 pack、计算 SHA-256，并原子生成 catalog metadata。生成后：

1. 检查 locale、native name、direction 和翻译质量。
2. 运行前端与 .NET 构建。
3. 运行 `LanguagePackArtifactTests`。
4. 从实际发布服务下载文件并再次核对 digest。

catalog 使用：

```json
{
  "schema": "leserpent.language-pack-catalog/v1",
  "packs": [
    {
      "locale": "pt-BR",
      "nativeName": "Português (Brasil)",
      "version": "1.0.0",
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
- `scripts/build-language-packs.mjs`
  - 30-locale roster 中 22 个下载包的翻译源、pack 生成与 digest catalog

建议在每次打包前先执行 `npm run verify:language-packs`，用于快速核对内置 locale 对齐和 22 个官方下载包的核心覆盖率。

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
- RTL pack 安装后 `document.dir=rtl`，卸载或切换 LTR 语言后恢复。
