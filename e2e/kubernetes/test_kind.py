"""A fresh install of the chart on a kind cluster with the bundled stores: the setup link, a
request to the mock provider that the request log records, and an upgrade that keeps serving.

Runs against the cluster of kubectl's current context, with the images CI built loaded into it
(`kind load docker-image`). Set KAESO_TEST_KIND=1 to run it."""

import os
import re
import socket
import subprocess
import time
from pathlib import Path

import httpx
import openai
import pytest

pytestmark = pytest.mark.skipif(os.environ.get("KAESO_TEST_KIND") != "1", reason="needs a kind cluster: set KAESO_TEST_KIND=1")

ROOT = Path(__file__).resolve().parents[2]
CHART = ROOT / "deploy" / "helm" / "kaeso"
NAMESPACE = "kaeso"
SETUP_LINK = re.compile(r"(\S+)#token=(\S+)")
# kind keeps a loaded image under Docker Hub's library name.
REGISTRY = "docker.io/library"


def tag_of(variable: str, default: str) -> tuple[str, str]:
    repository, tag = os.environ.get(variable, default).rsplit(":", 1)
    return repository, tag


def run(*args: str, **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(args, check=True, text=True, capture_output=True, **kwargs)


def kubectl(*args: str, **kwargs) -> subprocess.CompletedProcess:
    return run("kubectl", "--namespace", NAMESPACE, *args, **kwargs)


def free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def poll(fetch, what: str, seconds: float = 120):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        found = fetch()
        if found:
            return found
        time.sleep(1)
    raise TimeoutError(f"{what} did not happen within {seconds} seconds")


def diagnose() -> str:
    out = subprocess.run(
        ["kubectl", "--namespace", NAMESPACE, "get", "pods,jobs,events", "-o", "wide"], capture_output=True, text=True
    )
    logs = subprocess.run(
        ["kubectl", "--namespace", NAMESPACE, "logs", "--tail", "40", "-l", "app.kubernetes.io/component=gateway", "--all-containers"],
        capture_output=True,
        text=True,
    )
    return out.stdout + out.stderr + logs.stdout + logs.stderr


MOCK = """
apiVersion: apps/v1
kind: Deployment
metadata:
  name: mock
spec:
  selector:
    matchLabels: {app: mock}
  template:
    metadata:
      labels: {app: mock}
    spec:
      containers:
        - name: mock
          image: %(image)s
          imagePullPolicy: Never
          ports:
            - containerPort: 8080
---
apiVersion: v1
kind: Service
metadata:
  name: mock
spec:
  selector: {app: mock}
  ports:
    - port: 8080
"""


@pytest.fixture(scope="module")
def install():
    gateway, gateway_tag = tag_of("KAESO_TEST_GATEWAY_IMAGE", "kaeso-gateway:ci")
    console, console_tag = tag_of("KAESO_TEST_CONSOLE_IMAGE", "kaeso-console:ci")
    mock, mock_tag = tag_of("KAESO_TEST_MOCK_IMAGE", "kaeso-mock:ci")
    port = free_port()
    url = f"http://localhost:{port}"
    run("kubectl", "create", "namespace", NAMESPACE)
    kubectl("apply", "-f", "-", input=MOCK % {"image": f"{REGISTRY}/{mock}:{mock_tag}"})
    values = [
        "--set", f"publicUrl={url}",
        "--set", "bundled.enabled=true",
        "--set", f"image.registry={REGISTRY}",
        "--set", "image.pullPolicy=Never",
        "--set", f"gateway.image.repository={gateway}",
        "--set", f"gateway.image.tag={gateway_tag}",
        "--set", f"console.image.repository={console}",
        "--set", f"console.image.tag={console_tag}",
        "--set", "ingress.enabled=false",
        "--set", "mail.transport=log",
        "--set-string", "gateway.env.KAESO_KPMR_SYNC=false",
        "--set-string", "gateway.env.KAESO_UPDATES_CHECK=false",
        "--set-string", "gateway.env.KAESO_PROVIDERS_OPENAI_BASE_URL=http://mock:8080",
    ]
    forward = None
    try:
        try:
            installed = run("helm", "install", "kaeso", str(CHART), "--namespace", NAMESPACE, "--wait", "--timeout", "10m", *values)
        except subprocess.CalledProcessError as e:
            raise AssertionError(f"helm install failed: {e.stderr}\n{diagnose()}") from e
        forward = subprocess.Popen(
            ["kubectl", "--namespace", NAMESPACE, "port-forward", "service/kaeso-gateway", f"{port}:8080"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

        def healthy() -> bool:
            try:
                return httpx.get(f"{url}/healthz", timeout=2).status_code == 200
            except httpx.HTTPError:
                return False

        poll(healthy, "the gateway answering through port-forward", seconds=60)
        yield {"url": url, "values": values, "notes": installed.stdout}
    finally:
        if forward:
            forward.terminate()
        subprocess.run(["kubectl", "--namespace", NAMESPACE, "get", "all,events"], check=False)
        subprocess.run(["helm", "uninstall", "kaeso", "--namespace", NAMESPACE, "--wait"], check=False)
        subprocess.run(["kubectl", "delete", "namespace", NAMESPACE, "--wait=false"], check=False)


@pytest.fixture(scope="module")
def key(install) -> str:
    url = install["url"]
    printed = kubectl("exec", "deployment/kaeso-gateway", "--", "kaeso", "setup-link").stdout
    link = SETUP_LINK.search(printed)
    assert link, printed
    assert link.group(1).startswith(url), "the setup link opens the install's own address"
    http = httpx.Client(base_url=url, headers={"origin": url}, timeout=30)
    r = http.post(
        "/platform/v1/auth/setup",
        json={
            "token": link.group(2),
            "email": "owner@example.com",
            "name": "Owner",
            "password": "correct horse battery",
            "org_name": "First org",
        },
    )
    assert r.status_code == 200, r.text

    def post(path: str, body: dict) -> dict:
        r = http.post(f"/platform/v1{path}", json=body)
        assert r.status_code in (200, 201), f"{path}: {r.status_code} {r.text}"
        return r.json()

    org = http.get("/platform/v1/auth/session").json()["memberships"][0]["org_id"]
    project = post(f"/orgs/{org}/projects", {"name": "First project"})["id"]
    base = f"/orgs/{org}/projects/{project}"
    post(f"/orgs/{org}/credentials", {"provider": "openai", "name": "OpenAI", "secret": "sk-test-kind-openai"})
    post(f"{base}/models", {"provider": "openai", "model_id": "gpt-4o"})
    secret = post(f"{base}/keys", {"name": "kind"})["secret"]
    install["http"] = http
    install["base"] = base
    return secret


def test_the_notes_say_how_to_get_the_setup_link(install):
    assert "kaeso setup-link" in install["notes"]


def test_a_fresh_install_serves_a_request_and_records_it(install, key):
    client = openai.OpenAI(base_url=f"{install['url']}/v1", api_key=key, max_retries=0)
    reply = client.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "Hello"}])
    assert reply.choices[0].message.content
    http, base = install["http"], install["base"]
    poll(lambda: len(http.get(f"/platform/v1{base}/logs").json().get("data", [])) >= 1, "the request in the bundled ClickHouse", seconds=60)


LOAD = """
ok=0; failed=0; end=$(( $(date +%s) + 600 ))
while [ ! -f /tmp/stop ] && [ "$(date +%s)" -lt "$end" ]; do
  if wget -q -O /dev/null -T 30 --header "Authorization: Bearer $KEY" --header "Content-Type: application/json" \
      --post-data '{"model":"gpt-4o","messages":[{"role":"user","content":"Hi"}]}' http://kaeso-gateway:8080/v1/chat/completions; then
    ok=$((ok + 1))
  else
    failed=$((failed + 1))
  fi
  sleep 0.2
done
echo "ok=$ok failed=$failed"
"""


def test_an_upgrade_migrates_first_and_keeps_serving(install, key):
    console, console_tag = tag_of("KAESO_TEST_CONSOLE_IMAGE", "kaeso-console:ci")
    kubectl(
        "run", "load", f"--image={REGISTRY}/{console}:{console_tag}", "--image-pull-policy=Never", "--restart=Never",
        f"--env=KEY={key}", "--command", "--", "sh", "-c", LOAD,
    )
    kubectl("wait", "--for=condition=Ready", "pod/load", "--timeout=120s")
    time.sleep(5)
    try:
        run(
            "helm", "upgrade", "kaeso", str(CHART), "--namespace", NAMESPACE, "--wait", "--timeout", "10m", *install["values"],
            "--set-string", f"gateway.podAnnotations.kaeso-test/upgraded={int(time.time())}",
        )
    except subprocess.CalledProcessError as e:
        raise AssertionError(f"helm upgrade failed: {e.stderr}\n{diagnose()}") from e
    time.sleep(5)
    kubectl("exec", "load", "--", "touch", "/tmp/stop")
    kubectl("wait", "--for=jsonpath={.status.phase}=Succeeded", "pod/load", "--timeout=120s")
    counts = dict(part.split("=") for part in kubectl("logs", "load").stdout.split())
    assert int(counts["ok"]) > 20 and counts["failed"] == "0", counts
    jobs = kubectl("get", "jobs", "-o", "name").stdout
    assert "kaeso-migrate" not in jobs, "the migration ran and its Job went away once it succeeded"
