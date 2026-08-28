# 打包

使用 `apps/desktop` 的 electron-builder：

- Linux: `deb` / `rpm` / `tar.gz`
- Windows: `nsis`
- macOS: `dmg`

CI 产出**未签名**包。macOS 公证与 Windows 签名需在发行机器配置证书，不在默认构建里执行。

sidecar 二进制 `omegat-sidecar` 与 CLI `omegat` 由 `cargo +stable build --release -p omegat-sidecar -p omegat-cli` 提供，electron-builder `extraResources` 拷入。

SVN 团队功能依赖系统 `svn` 客户端，发行说明需写明。
