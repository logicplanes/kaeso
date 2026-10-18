---
title: 架构
description: 网关、控制台、文档站点、邮件和更新器的内部构建方式。
order: 8
---

这些页面描述 Kaeso 各部分的构建方式，面向即将修改它们的贡献者。[简介](../../index.md#各部分如何配合)展示了各部分如何配合，[请求流程](../../concepts/request-flow.md)则从外部跟随一个请求。

## 下一步

- [网关](gateway.md)：跟随一个请求穿过各个 crate，并了解 Rust 代码遵循的规则。
- [控制台](console.md)：了解控制台如何读取数据、检查权限、加载代码和显示错误。
- [文档站点](docs-site.md)：修改 Markdown 页面生成 docs.kaeso.app 的方式。
- [邮件](emails.md)：修改网关发送的邮件。
- [更新](updates.md)：修改网关查找版本的方式，以及更新器验证和应用版本的方式。
