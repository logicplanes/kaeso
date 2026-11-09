---
title: Azure
description: Run Kaeso on Azure on one virtual machine with the Compose package, from the Deploy to Azure button or the Azure CLI.
order: 6
---

`deploy/azure/` in the repository holds an Azure Resource Manager template for a quick server: the Compose package on one virtual machine, as on your own server. `createUiDefinition.json` beside it shapes the portal's form for the template.

## What the template creates

- a virtual network with one subnet and a network security group that lets in HTTP, HTTPS and HTTP/3 only, and SSH only from an address range you give;
- a static public address with a DNS name under `<region>.cloudapp.azure.com`;
- an Ubuntu 24.04 virtual machine with Trusted Launch, boot diagnostics and a 40 GB disk;
- a Custom Script extension that runs the installer at the first boot. It installs Docker, writes the settings and starts the Compose package with Caddy, bundled ClickHouse and the updater ([installation](../installation.md)).

The deployment finishes once the installer has. Without a domain of your own, Kaeso answers on the address's DNS name, and Caddy gets its certificate for that name.

## Deploy with the button

1. Open [Deploy to Azure](https://portal.azure.com/#create/Microsoft.Template/uri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2Fquick-server.json/createUIFormUri/https%3A%2F%2Fraw.githubusercontent.com%2Flogicplanes%2Fkaeso%2Fprod%2Fdeploy%2Fazure%2FcreateUiDefinition.json) and sign in. The portal reads the template from the branch `prod`, which holds the latest release.
2. Choose a subscription, a new resource group and a region, and fill in the [parameters](#parameters).
3. Create the deployment, and wait until it is complete, about five minutes.
4. Under the deployment's **Outputs**, copy `setupLink` and run it in [Azure Cloud Shell](https://shell.azure.com) or with the Azure CLI. It prints the one-time setup link without SSH; open it to create the first user ([the first start](../how-it-works.md#the-first-start)).
5. With a domain of your own, point its CNAME record at the `fqdn` output, or an A record at `publicIp`. Caddy keeps trying until the name resolves, then gets the certificate.

## Deploy with the Azure CLI

From a clone of the repository, create a resource group and deploy the template into it:

```bash
az group create --name kaeso --location westeurope
az deployment group create --resource-group kaeso --template-file deploy/azure/quick-server.json \
  --parameters domain=kaeso.example.com adminPasswordOrKey="$(cat ~/.ssh/id_ed25519.pub)" sshSourceAddress=203.0.113.0/24
```

The command prints the outputs once the deployment is complete. Run `setupLink` from them as in the button's step 4.

## Parameters

| Parameter | What it is |
| --- | --- |
| `domain` | The domain Kaeso answers on; empty uses the address's DNS name |
| `dnsLabel` | The address's DNS name under `<region>.cloudapp.azure.com`; empty makes one from the resource group |
| `kaesoVersion` | A release to install; empty installs the newest stable release |
| `vmSize`, `diskSizeGb`, `diskType` | `Standard_B2als_v2` with 2 vCPUs and 4 GB, and a 40 GB Standard SSD, by default ([requirements](../requirements.md)) |
| `adminUsername`, `authenticationType`, `adminPasswordOrKey` | The server's administrator and an SSH public key or a password, for SSH and the serial console |
| `sshSourceAddress` | An address range that may reach SSH; empty keeps port 22 closed |
| `location` | The region; the resource group's by default |

The template has no secret of Kaeso's: the installer generates the master key and the passwords on the server. Keep a copy of `/opt/kaeso/master-keys` apart from your backups; `setupLink` with `cat /opt/kaeso/master-keys` as its script prints it ([backups and master keys](../backups.md)).

## Update

Updates work as on any Compose install: from the console, automatically, or by hand ([upgrades](../upgrades.md)). The template only creates the server; deploying it again doesn't update Kaeso.

## Limits

- The template is a quick server only. The repository has no template for Azure Container Apps with Azure Database for PostgreSQL and a managed cache.
- The sizes are x64 ones. For an Arm64 size, deploy Ubuntu 24.04 for Arm64 and run the installer by hand.

## What this is based on

The template follows the Resource Manager references for virtual machines, the Custom Script extension, public addresses and network security groups, and Microsoft's page on the Deploy to Azure button, read on 2026-11-09. It passes all 49 tests of the [ARM template test toolkit](https://github.com/Azure/arm-ttk), which also check `createUiDefinition.json`. It hasn't been deployed on Azure, and `az deployment group validate` needs a signed-in account, so it wasn't run. Deploy it in a disposable resource group first, and check the setup link, a streamed completion and the client address in a request log ([what was validated](index.md#what-was-validated)).

## Next steps

- [Installation](../installation.md): What the installer sets up on the server, and how to change it.
- [Backups and master keys](../backups.md): Back up the server and keep the master key outside Azure.
- [Upgrades](../upgrades.md): Update Kaeso from the console or by hand.
