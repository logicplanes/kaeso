import shutil
import subprocess
import uuid
from pathlib import Path

import httpx
import pytest

from test_install import free_port, poll

ROOT = Path(__file__).resolve().parents[2]
DOCKERFILE = ROOT / "deploy" / "console.Dockerfile"
CONTEXT = [
    "deploy/console.Dockerfile",
    "deploy/console.Dockerfile.dockerignore",
    "deploy/console/Caddyfile",
    "deploy/console/serve.sh",
]
DIST = {
    "index.html": '<!doctype html><div id="root"></div>',
    "favicon.svg": "<svg xmlns='http://www.w3.org/2000/svg'/>",
    "assets/index-a1b2c3d4.js": "export {};",
}


def closed(path: Path) -> None:
    path.chmod(0o700 if path.is_dir() else 0o600)


def checkout_with_umask_077(base: Path) -> tuple[Path, Path]:
    context, build = base / "context", base / "build"
    for name in CONTEXT:
        (context / name).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, context / name)
    dist = build / "app" / "apps" / "console" / "dist"
    for name, text in DIST.items():
        (dist / name).parent.mkdir(parents=True, exist_ok=True)
        (dist / name).write_text(text)
    for tree in (context, build):
        for path in [tree, *tree.rglob("*")]:
            closed(path)
    return context, build


@pytest.fixture(scope="module")
def image(tmp_path_factory):
    context, build = checkout_with_umask_077(tmp_path_factory.mktemp("console"))
    tag = f"kaeso-console:umask-{uuid.uuid4().hex[:8]}"
    subprocess.run(
        ["docker", "build", "-q", "-f", str(context / "deploy" / "console.Dockerfile")]
        + ["--build-context", f"build={build}", "-t", tag, str(context)],
        check=True,
        capture_output=True,
        text=True,
    )
    try:
        yield tag
    finally:
        subprocess.run(["docker", "image", "rm", "-f", tag], check=False, capture_output=True)


def test_an_image_built_from_a_umask_077_checkout_serves_the_console(image):
    port = free_port()
    name = f"kaeso-console-umask-{uuid.uuid4().hex[:8]}"
    subprocess.run(["docker", "run", "-d", "--name", name, "-p", f"127.0.0.1:{port}:3000", image], check=True, capture_output=True)
    try:
        url = f"http://127.0.0.1:{port}"

        def healthy() -> bool:
            try:
                return httpx.get(f"{url}/healthz", timeout=2).text == "ok"
            except httpx.HTTPError:
                return False

        try:
            poll(healthy, "the console answering /healthz", seconds=30)
        except TimeoutError as error:
            logs = subprocess.run(["docker", "logs", name], capture_output=True, text=True)
            files = subprocess.run(["docker", "run", "--rm", "--entrypoint", "ls", image, "-lnR", "/etc/caddy", "/srv"], capture_output=True, text=True)
            raise AssertionError(f"{error}; the container logged: {logs.stdout}{logs.stderr}; its files: {files.stdout}") from error
        for path, text in DIST.items():
            reply = httpx.get(f"{url}/{path}", timeout=5)
            assert reply.status_code == 200, f"/{path}: {reply.status_code}"
            assert reply.text == text
        assert httpx.get(f"{url}/keys", timeout=5).text == DIST["index.html"]
        files = subprocess.run(
            ["docker", "exec", name, "stat", "-c", "%a %U %n", "/etc/caddy/Caddyfile", "/srv", "/srv/index.html", "/srv/assets"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        assert files == ["444 root /etc/caddy/Caddyfile", "755 root /srv", "444 root /srv/index.html", "555 root /srv/assets"]
    finally:
        subprocess.run(["docker", "rm", "-f", name], check=False, capture_output=True)


def test_the_console_stops_with_a_message_when_it_cannot_read_its_files(image):
    run = subprocess.run(
        ["docker", "run", "--rm", "--tmpfs", "/srv:mode=0700", image],
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert run.returncode == 1
    assert "kaeso-console: user console cannot read these files, so the console cannot start" in run.stderr
    assert "/srv" in run.stderr


def test_the_console_image_sets_the_permissions_of_every_file_it_copies():
    lines = DOCKERFILE.read_text().splitlines()
    final = lines[max(i for i, line in enumerate(lines) if line.startswith("FROM ")) :]
    copies = [line for line in final if line.startswith("COPY ")]
    assert copies and all("--chmod=" in line for line in copies), copies
    assert final.index("USER console") < final.index(next(line for line in final if line.startswith("RUN caddy validate")))
    assert final[-1] == 'CMD ["/usr/local/bin/kaeso-console"]'


@pytest.mark.parametrize("target", ["mock", "gateway"])
def test_the_gateway_images_set_the_permissions_of_every_file_they_copy(target):
    lines = (ROOT / "deploy" / "gateway.Dockerfile").read_text().splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("FROM ") and line.endswith(f" AS {target}"))
    end = next((i for i, line in enumerate(lines[start + 1 :], start + 1) if line.startswith("FROM ")), len(lines))
    copies = [line for line in lines[start:end] if line.startswith("COPY ")]
    assert copies and all("--chmod=" in line for line in copies), copies
