# End-to-end tests

The official OpenAI, Anthropic, Google Gen AI, ElevenLabs, TypeSafe and MCP Python SDKs, pinned in `e2e/requirements.txt`, against KPP's encoders (`e2e/conformance`, served by `crates/kpp-conformance`), the gateway (`e2e/sdk`), its MCP portal (`e2e/mcp`), a fresh install from `deploy/compose.yaml` (`e2e/install`) and the Helm chart in `deploy/helm/kaeso` (`e2e/kubernetes`). How to set them up and run them is in the documentation: [Testing › End-to-end tests](https://docs.kaeso.app/development/testing#end-to-end-tests).
