---
title: 自托管
description: 在自己的服务器上运行 Kaeso，并保持其配置正确、版本最新、数据有备份。
order: 5
---

Kaeso 以一个 Docker Compose 项目的形式运行在您的服务器上。[快速开始](../get-started/quick-start.md)负责安装，之后的一切都在这些页面中。

## 搭建

- [要求](requirements.md)：确定服务器规格、开放网络，并为请求日志规划磁盘。
- [实例的工作原理](how-it-works.md)：了解运行哪些服务、各自保存什么，以及它们如何相互隔离。
- [安装](installation.md)：安装指定版本、在自己的电脑上试用，并运行 Kaeso 的命令。
- [配置](configuration.md)：查找每项设置及其所在位置。
- [域名和 TLS](domains-and-tls.md)：在自己的域名下提供 Kaeso，或放在已有的代理后面。
- [电子邮件](email.md)：连接 SMTP 服务器，并了解 Kaeso 发送哪些邮件。
- [登录和单点登录](sign-in.md)：选择登录方式，并在所有方式都失效时重新登录。

## 运维

- [升级](upgrades.md)：手动或一键更新到新版本。
- [备份和主密钥](backups.md)：备份数据库和主密钥，并轮换主密钥。
- [健康状态和指标](monitoring.md)：检查健康状态、抓取 Prometheus 指标并阅读日志。
- [故障排除](troubleshooting.md)：为遇到的问题找到解决方法。
