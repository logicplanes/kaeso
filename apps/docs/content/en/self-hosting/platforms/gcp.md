---
title: Google Cloud
description: Run Kaeso on Google Cloud: one Compute Engine server with the Compose package, or Cloud Run with a load balancer, Cloud SQL and Memorystore.
order: 5
---

`deploy/gcp/` in the repository holds two Terraform configurations, which OpenTofu also applies. The quick server is the Compose package on one virtual machine, as on your own server. The managed configuration is Kaeso on Cloud Run with Google's data stores, for installs that outgrow one machine.

## Quick server

`deploy/gcp/quick-server/` creates one Compute Engine instance with Ubuntu 24.04, a static address, firewall rules for HTTP, HTTPS and HTTP/3, SSH from address ranges you choose, and an optional Cloud DNS record. At first boot, the instance runs the installer, which installs Docker, writes the settings and starts the Compose package with Caddy, bundled ClickHouse and the updater ([installation](../installation.md)). [Open in Cloud Shell](https://shell.cloud.google.com/cloudshell/editor?cloudshell_git_repo=https://github.com/logicplanes/kaeso&cloudshell_git_branch=prod&cloudshell_workspace=deploy/gcp/quick-server) clones the `prod` branch, which holds the latest release, with this folder as the workspace; the commands below run there with `terraform` in place of `tofu`, which Cloud Shell doesn't install.

```bash
cd deploy/gcp/quick-server
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com -var 'ssh_source_ranges=["203.0.113.0/24"]'
```

| Variable | What it is |
| --- | --- |
| `project`, `region`, `zone` | Where the instance runs; `europe-west3-a` by default |
| `domain` | The domain Kaeso answers on; without `dns_zone`, point its A record at the `public_ip` output |
| `kaeso_version` | A release to install; empty installs the newest stable release |
| `machine_type`, `disk_size_gb` | `e2-medium` and 40 GB by default ([requirements](../requirements.md)) |
| `ssh_source_ranges` | Who may reach SSH, which `kaeso setup-link` and upgrades by hand need |
| `dns_zone` | A Cloud DNS managed zone of the domain, to create its record |

Once the instance is up and the certificate is issued, run the `setup_link` output to print the one-time setup link. Updates work as on any Compose install: from the console, automatically, or by hand ([upgrades](../upgrades.md)).

## Managed production

`deploy/gcp/managed/` creates:

- a VPC with a subnet, a serverless VPC connector and private services access;
- Cloud SQL for PostgreSQL 17 with a private address only, and Memorystore for Valkey with `noeviction`, reachable only over the VPC;
- Secret Manager secrets for the master keys, the PostgreSQL URL, the SMTP password and the ClickHouse URL, read by the gateway's service account;
- Cloud Run services for the gateway (2 CPUs and 4 GB by default, at least one instance kept warm, a request timeout of an hour) and the console, reachable only through the load balancer;
- a global external Application Load Balancer with a managed certificate, HTTP redirected to HTTPS, and path rules that send `/v1/*`, `/v1beta/*`, `/v2/*`, `/platform/*`, `/mcp`, `/mcp/*`, `/oauth/*` and `/.well-known/oauth-*` to the gateway and everything else to the console, with a backend timeout of an hour for long streams;
- an optional Cloud DNS record.

```bash
cd deploy/gcp/managed
tofu init
tofu apply -var project=my-project -var domain=kaeso.example.com \
  -var "master_keys=$(docker run --rm ghcr.io/logicplanes/kaeso-gateway:{{version}} master-key generate)" \
  -var setup_email=owner@example.com -var smtp_host=smtp.example.com -var smtp_user=kaeso -var smtp_password=secret \
  -var 'smtp_from=Kaeso <kaeso@example.com>'
```

| Variable | What it is |
| --- | --- |
| `project`, `region`, `name` | Where everything runs and the prefix of every resource |
| `domain`, `dns_zone` | The domain, and a Cloud DNS zone to create its record |
| `master_keys` | The master keys; the configuration keeps them in Secret Manager, and you keep a copy in a password manager |
| `setup_email` | Where the gateway emails the one-time setup link until the first user exists |
| `clickhouse_url` | A ClickHouse HTTP URL with user and password, such as ClickHouse Cloud; empty keeps request logs off |
| `smtp_…` | Your SMTP server ([email](../email.md)) |
| `gateway_cpu`, `gateway_memory`, `gateway_min_instances`, `gateway_max_instances` | The gateway's size and how many instances run; they share limits, caches and route cooldowns through Valkey |
| `database_tier`, `database_disk_gb`, `cache_node_type` | The data stores' sizes |

Memorystore is reached without a password and without TLS, over the private network only, because the gateway can't present Google's IAM tokens or its private certificate authority. The gateway trusts `X-Forwarded-For` from the load balancer's proxy ranges and Cloud Run's own front end, and keeps its metrics on its loopback address. The managed certificate is issued once the domain points at the load balancer's address, which takes up to an hour.

## Update and roll back

A release changes the image tags in `main.tf`. Apply the new configuration; Cloud Run creates new revisions and shifts traffic once they are healthy, and the gateway migrates the databases at start, forward only ([upgrades](../upgrades.md)). To roll back on purpose, apply the previous release's configuration or shift traffic to the previous revision in Cloud Run; a release whose migrations the earlier version can't run stays.

## What this is based on

Both configurations pass `tofu fmt` and `tofu validate` with the Google provider, which CI runs. On 2026-11-07 the quick-server configuration was applied in a new project in `europe-west3` and installed the eighth release candidate. The setup link, HTTPS health and readiness checks, console and API routing, a streamed completion, the ClickHouse request log and the replacement of a forged `X-Forwarded-For` all passed ([what was validated](index.md#what-was-validated)). The managed configuration has only been validated; apply it in a disposable project before relying on it.

## Next steps

- [Configuration](../configuration.md): Every setting the gateway reads, and what each one does.
- [Backups and master keys](../backups.md): Back up Cloud SQL and keep the master key outside Google Cloud.
- [Health and metrics](../monitoring.md): The health checks Cloud Run and the load balancer use.
