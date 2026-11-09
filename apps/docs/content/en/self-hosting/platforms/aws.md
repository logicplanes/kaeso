---
title: AWS
description: Run Kaeso on AWS: one EC2 server with the Compose package, or ECS Fargate with an Application Load Balancer, RDS and ElastiCache.
order: 4
---

`deploy/aws/` in the repository holds two CloudFormation templates. The quick server is the Compose package on one instance, as on your own server. The managed stack is Kaeso on ECS Fargate with AWS data stores, for installs that outgrow one machine.

## Quick server

[Launch on AWS](https://console.aws.amazon.com/cloudformation/home#/stacks/create/review?templateURL=https%3A%2F%2Fkaeso-cloudformation.s3.amazonaws.com%2Flatest%2Fquick-server.yaml&stackName=kaeso) opens the published quick-server template in CloudFormation. The same file is at `deploy/aws/quick-server.yaml` in the repository. Every release uploads both templates to `https://kaeso-cloudformation.s3.amazonaws.com/vX.Y.Z/`, and a stable release to `latest/` ([releasing](../../development/releasing.md)).

`deploy/aws/quick-server.yaml` creates one Graviton instance with Ubuntu 24.04, an Elastic IP, a security group for HTTP, HTTPS and HTTP/3, SSH from an address range you choose, and an optional Route 53 record. At first boot, the instance runs the installer, which installs Docker, writes the settings and starts the Compose package with Caddy, bundled ClickHouse and the updater ([installation](../installation.md)).

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/quick-server.yaml \
  --parameter-overrides Domain=kaeso.example.com KeyName=my-key AllowedSshCidr=203.0.113.0/24 HostedZoneId=Z0123456789
```

| Parameter | What it is |
| --- | --- |
| `Domain` | The domain Kaeso answers on; without `HostedZoneId`, point its A record at the `PublicIp` output |
| `KaesoVersion` | A release to install; empty installs the newest stable release |
| `InstanceType`, `DiskSizeGb` | `t4g.medium` and 40 GB by default ([requirements](../requirements.md)) |
| `KeyName`, `AllowedSshCidr` | SSH access, which `kaeso setup-link` and upgrades by hand need |
| `HostedZoneId` | A Route 53 zone of the domain, to create its record |

Once the stack is complete and the certificate is issued, run the `SetupLink` output over SSH to print the one-time setup link. Updates work as on any Compose install: from the console, automatically, or by hand ([upgrades](../upgrades.md)).

## Managed production

`deploy/aws/managed.yaml` creates:

- a VPC with two public and two private subnets and one NAT gateway;
- an Application Load Balancer with your ACM certificate, HTTP redirected to HTTPS, path rules that send `/v1/*`, `/v1beta/*`, `/v2/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*` and `/.well-known/oauth-*` to the gateway and everything else to the console, and an idle timeout of 300 seconds for long streams;
- an ECS cluster with a gateway service (2 vCPUs and 4 GB by default, 2 tasks) and a console service, on Fargate with Graviton, in the private subnets;
- RDS PostgreSQL 18 and ElastiCache Valkey with `noeviction`, TLS and an AUTH token, reachable only from the gateway;
- Secrets Manager secrets for the master keys, the database password, the Valkey token, the SMTP password and the ClickHouse URL;
- CloudWatch log groups, and an optional Route 53 alias.

```bash
aws cloudformation deploy --stack-name kaeso --template-file deploy/aws/managed.yaml --capabilities CAPABILITY_IAM \
  --parameter-overrides Domain=kaeso.example.com CertificateArn=arn:aws:acm:eu-central-1:123456789012:certificate/example \
  MasterKeys="$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  SetupEmail=owner@example.com SmtpHost=smtp.example.com SmtpUser=kaeso SmtpPassword=secret SmtpFrom="Kaeso <kaeso@example.com>"
```

| Parameter | What it is |
| --- | --- |
| `Domain`, `CertificateArn` | The domain and an ACM certificate for it in the stack's region; `HostedZoneId` creates the alias record |
| `MasterKeys` | The master keys; the stack keeps them in Secrets Manager, and you keep a copy in a password manager |
| `SetupEmail` | Where the gateway emails the one-time setup link until the first user exists |
| `ClickHouseUrl` | A ClickHouse HTTP URL with user and password, such as ClickHouse Cloud; empty keeps request logs off |
| `Smtp…` | Your SMTP server ([email](../email.md)) |
| `GatewayCpu`, `GatewayMemory`, `GatewayCount` | The gateway's size and how many tasks run; they share limits, caches and route cooldowns through Valkey |
| `DatabaseInstanceClass`, `DatabaseStorageGb`, `CacheNodeType` | The data stores' sizes |

The gateway trusts `X-Forwarded-For` from the VPC, where the load balancer's nodes sit, and keeps its metrics on its loopback address. The load balancer checks `/readyz` on the gateway and `/healthz` on the console; a gateway task gets 75 seconds to drain when it stops.

## Update and roll back

A release changes the image tags in `managed.yaml`. Deploy the new template over the stack; ECS replaces the tasks with a circuit breaker that rolls back on failure, and the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back on purpose, deploy the previous release's template; a release whose migrations the earlier version can't run stays.

## What this is based on

Both templates pass `cfn-lint`, which CI runs. On 2026-11-07 the published quick-server template created a stack from an empty AWS account in `us-east-1` and installed the eighth release candidate. Its first boot exposed an ownership bug: the template reclaimed `/opt/kaeso` after installation, so the gateway could not read the master-key file. After restoring ownership on that test instance, the setup link, HTTPS health and readiness checks, console and API routing, a streamed completion, the ClickHouse request log and the replacement of a forged `X-Forwarded-For` passed ([what was validated](index.md#what-was-validated)). The template now leaves the installer-owned file alone, and a regression check rejects the old `chown`; the corrected template has not been redeployed from an empty account. The managed stack has only been linted; deploy it in a disposable account before relying on it.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up RDS and keep the master key outside AWS.
- [Health and metrics](../monitoring.md): The health checks the load balancer uses.
