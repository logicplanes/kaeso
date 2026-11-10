---
title: Azure
description: 在 Azure 上用一台运行 Compose 包的虚拟机运行 Kaeso，通过 Deploy to Azure 按钮或 Azure CLI 部署。
order: 6
---

仓库中的 `deploy/azure/` 包含一个用于快速服务器的 Azure 资源管理器模板：在一台虚拟机上运行 Compose 包，与在您自己的服务器上一样。旁边的 `createUiDefinition.json` 决定门户为该模板显示的表单。

## 模板创建的内容

- 一个虚拟网络，含一个子网和一个网络安全组，只允许 HTTP、HTTPS 和 HTTP/3 进入，SSH 只允许来自您指定的地址范围；
- 一个静态公网地址，其 DNS 名称位于 `<region>.cloudapp.azure.com` 之下；
- 一台 Ubuntu 24.04 虚拟机，启用受信任启动和启动诊断，磁盘 40 GB；
- 一个 Custom Script 扩展，在首次启动时运行安装器。它安装 Docker、写入设置，并启动带 Caddy、内置 ClickHouse 和更新器的 Compose 包（[安装](../installation.md)）。

安装器完成后部署才结束。没有自己的域名时，Kaeso 使用该地址的 DNS 名称应答，Caddy 为该名称获取证书。

## 用按钮部署

1. 打开 [Deploy to Azure](https://portal.azure.com/#create/Microsoft.Template/uri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2Fquick-server.json/createUIFormUri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2FcreateUiDefinition.json) 并登录。门户从包含最新版本的 `prod` 分支读取模板。
2. 选择订阅、新的资源组和区域，并填写[参数](#参数)。
3. 创建部署，等待它完成，大约需要五分钟。
4. 在部署的 **Outputs** 下复制 `setupLink`，在 [Azure Cloud Shell](https://shell.azure.com) 中或用 Azure CLI 运行。它无需 SSH 即可输出一次性设置链接；打开它创建第一个用户（[首次启动](../how-it-works.md#首次启动)）。
5. 使用自己的域名时，把它的 CNAME 记录指向 `fqdn` 输出，或把 A 记录指向 `publicIp`。Caddy 会一直重试，直到名称可以解析，然后获取证书。

## 用 Azure CLI 部署

在仓库的克隆中创建资源组，并把模板部署到其中：

```bash
az group create --name kaeso --location westeurope
az deployment group create --resource-group kaeso --template-file deploy/azure/quick-server.json \
  --parameters domain=kaeso.example.com adminPasswordOrKey="$(cat ~/.ssh/id_ed25519.pub)" sshSourceAddress=203.0.113.0/24
```

部署完成后，该命令会输出各项输出值。像按钮部署的第 4 步一样运行其中的 `setupLink`。

## 参数

| 参数 | 含义 |
| --- | --- |
| `domain` | Kaeso 应答的域名；留空则使用该地址的 DNS 名称 |
| `dnsLabel` | 地址在 `<region>.cloudapp.azure.com` 之下的 DNS 名称；留空则根据资源组生成 |
| `kaesoVersion` | 要安装的版本；留空则安装最新的稳定版本 |
| `vmSize`、`diskSizeGb`、`diskType` | 默认为 2 个 vCPU、4 GB 内存的 `Standard_B2als_v2` 和 40 GB 的标准 SSD（[要求](../requirements.md)） |
| `adminUsername`、`authenticationType`、`adminPasswordOrKey` | 服务器的管理员，以及 SSH 公钥或密码，用于 SSH 和串行控制台 |
| `sshSourceAddress` | 允许访问 SSH 的地址范围；留空则保持 22 端口关闭 |
| `location` | 区域；默认为资源组所在区域 |

模板不包含 Kaeso 的任何机密：安装器在服务器上生成主密钥和密码。把 `/opt/kaeso/master-keys` 的副本与备份分开保存；把 `setupLink` 的脚本换成 `cat /opt/kaeso/master-keys` 即可输出它（[备份和主密钥](../backups.md)）。

## 更新

更新方式与任何 Compose 安装相同：在控制台中、自动或手动进行（[升级](../upgrades.md)）。模板只创建服务器；再次部署它不会更新 Kaeso。

## 限制

- 该模板只是快速服务器。仓库中没有使用 Azure Container Apps、Azure Database for PostgreSQL 和托管缓存的模板。
- 可选规格都是 x64 规格。要使用 Arm64 规格，请部署 Arm64 版 Ubuntu 24.04，并手动运行安装器。

## 依据

该模板遵循资源管理器中虚拟机、Custom Script 扩展、公网地址和网络安全组的参考，以及 Microsoft 关于 Deploy to Azure 按钮的页面，均于 2026 年 11 月 9 日阅读。它通过了 [ARM 模板测试工具包](https://github.com/Azure/arm-ttk)的全部 49 项测试，这些测试也会检查 `createUiDefinition.json`。它尚未在 Azure 上部署；`az deployment group validate` 需要已登录的账户，因此没有运行。请先在一个临时资源组中部署，并检查设置链接、流式回复以及请求日志中的客户端地址（[已验证的内容](index.md#已验证的内容)）。

## 下一步

- [安装](../installation.md)：安装器在服务器上设置的内容，以及如何修改。
- [备份和主密钥](../backups.md)：备份服务器，并把主密钥保存在 Azure 之外。
- [升级](../upgrades.md)：在控制台中或手动更新 Kaeso。
