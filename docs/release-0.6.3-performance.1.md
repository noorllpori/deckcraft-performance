# DeckCraft 性能版 0.6.3-performance.1

发布日期：2026-10-19。独立性能分支发行版，仅维护 `performance`，不并入上游或本仓库主线。

## 这次更新

- 字体检查器改为紧凑列表，支持搜索、需注意项筛选、缺失/嵌入状态和替代字体诊断。点击使用页码跳转到对应幻灯片。
- 默认在 `.deckcraft` / `.pptx` 中打包允许编辑嵌入的静态 TrueType 字体，支持 TTC 抽取和 PPTX EOT 字体部件；可按文稿关闭。
- 修复微软雅黑、等线等本地化字体名称匹配。缺失字体、限制嵌入、暂不支持的格式会说明原因。
- PPTX 导出把支持的 PSD 合成图、WebP、BMP、TIFF 转成内嵌 PNG；PNG/JPEG/GIF 和可解码 EMF/WMF 保留原始字节。外链图片打包当前已接受的画面。
- 无法显示的图片或对象出现可见提示；保存成功后返回编辑画面。
- 保留高性能编辑、拖动代理、放映预编译/自动缓存、PSD 外链哈希刷新、中键平移和中心缩放。

## Windows 下载与升级

- `deckcraft-performance-0.6.3-performance.1-windows-x64.msi`：Windows x64 安装版，沿用原 UpgradeCode、安装目录和配置，升级已安装的 DeckCraft/性能版。
- `deckcraft-performance-0.6.3-performance.1-windows-x64-portable.zip`：Windows x64 便携包，解压后运行 `deckcraft.exe`，含命令行工具。
- `SHA256SUMS.txt`：附件 SHA-256 校验值。

软件显示名为 DeckCraft 性能版。安装包未做 Authenticode 数字签名。安装包和便携包包含许可证、NOTICE、素材归属与性能版贡献说明。不要在有未保存文稿时替换正在运行的程序。

## 验证与边界

- 全工作区测试：900 项通过，0 失败（含 49 项 UI 测试和文档测试）。格式、素材归属、28 个工作区 crate 的层级检查及 wasm 检查通过。
- 字体检查器和点击页码导航已在 Windows 调试版实际操作验证；已有 PowerPoint 抽查覆盖部分页面及 PSD/WebP/损坏图片样例。
- 严格 Clippy 仍未全绿：Rust 1.95 报出已有媒体/工具模块和界面代码的风格警告。未宣称完整 CI 通过。
- 未完成 WPS、所有页面/动画、跨机字体嵌入验证或性能对照跑分；不承诺固定提速倍数。完整字库会增加文件体积，字体许可限制仍然适用。可变字体/CFF 等嵌入格式仍有限制。
- 文件被 PowerPoint/WPS 占用时无法覆盖保存，需要关闭对应文稿或另存为新文件。

详细文档：[字体打包与检查器](font-embedding.md)、[PPTX 兼容导出](pptx-compatibility.md)、[性能与限制](performance-edition.md)。

## 贡献归属

**性能版贡献：葡萄拌大蒜（[@noorllpori](https://github.com/noorllpori)） · [B 站主页](https://space.bilibili.com/38222138)。**

**基础版贡献：Storytold DeckCraft / 原始作者与社区贡献者。** 保留基础版许可证与贡献列表；应用“关于”中与性能版贡献分开显示。本版本不是上游官方发行版。
