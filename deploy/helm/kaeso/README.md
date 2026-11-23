# Kaeso Helm chart

[Kaeso](https://kaeso.app) is the open-source AI gateway you run yourself: one OpenAI-, Anthropic-, Gemini- and ElevenLabs-compatible API in front of every provider, with single sign-on, roles, budgets, guardrails, alias routes, request logs, a playground and an MCP portal.

This chart runs the gateway and the console on Kubernetes 1.30 or later, with Helm 3.14 or later. Each release publishes it as `oci://ghcr.io/logicplanes/charts/kaeso`, with signed build provenance, and pins it to that release's images.

## What it creates

- The gateway, 2 replicas, non-root with a read-only root file system, ready on `/readyz`, and a metrics Service on port 9100 that no Ingress reaches. A `ServiceMonitor` and a HorizontalPodAutoscaler are optional.
- The console.
- A migration Job that runs once, as a Helm hook, before every upgrade, and before an install with your own stores, reaches the gateways.
- An Ingress, or an HTTPRoute for the Gateway API, that sends the APIs to the gateway and every other path to the console.
- PodDisruptionBudgets, and a Secret with the master keys the chart made, kept on upgrade and uninstall.
- With `bundled.enabled`, single copies of PostgreSQL, ClickHouse and Valkey for trying Kaeso out. In production, bring your own.

## Install

Create the namespace, put each store's connection URL in a Secret, and write your values with at least `publicUrl`, the stores and an SMTP server (`mail.smtp`), as step 2 of the [Kubernetes guide](https://docs.kaeso.app/self-hosting/platforms/kubernetes#install) shows. Then:

```bash
helm install kaeso oci://ghcr.io/logicplanes/charts/kaeso \
  --namespace kaeso --values values.yaml --wait
kubectl --namespace kaeso exec deployment/kaeso-gateway -- kaeso setup-link
```

The second command prints the one-time link that creates the first user. To only try it, `--create-namespace --set publicUrl=https://kaeso.example.com --set bundled.enabled=true --set mail.transport=log` needs no Secrets and nothing outside the cluster but the Ingress.

`helm show values oci://ghcr.io/logicplanes/charts/kaeso` lists every value with what it does, and the chart checks values against its schema, so a misspelled one fails the install.

## Upgrade

Read the release notes and back up the stores, then run `helm upgrade` with `--reset-then-reuse-values`. The migration Job runs first, and an upgrade whose migration fails changes nothing.

## Documentation

[Kubernetes](https://docs.kaeso.app/self-hosting/platforms/kubernetes) walks through the install step by step: the Secrets, master keys, routing and long streams, settings, scaling, metrics, upgrades and uninstalling.

## License

MIT. The Kaeso name and logo are not covered by the license.
