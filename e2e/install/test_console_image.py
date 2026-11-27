import re
import shutil
import subprocess
import uuid
from contextlib import contextmanager
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
    "deploy/console/edge.Caddyfile",
    "deploy/console/serve.sh",
    "deploy/console/edge.sh",
]
ROBOTS_META = '<!--{{with .RespHeader.Get "X-Robots-Tag"}}--><meta name="robots" content="{{html .}}" /><!--{{end}}-->'
NOINDEX = "noindex, nofollow"
SHARE = "/share/" + "A" * 43
DIST = {
    "index.html": f'<!doctype html><head>{ROBOTS_META}</head><div id="root"></div>',
    "share.html": f'<!doctype html><head>{ROBOTS_META}<meta property="og:type" content="website" /></head><div id="root"></div>',
    "favicon.svg": "<svg xmlns='http://www.w3.org/2000/svg'/>",
    "assets/index-a1b2c3d4.js": "export {};",
}


def page(robots: str | None, file: str = "index.html") -> str:
    meta = f'<!----><meta name="robots" content="{robots}" /><!---->' if robots else "<!---->"
    return DIST[file].replace(ROBOTS_META, meta)


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
    built = subprocess.run(
        ["docker", "build", "-q", "-f", str(context / "deploy" / "console.Dockerfile")]
        + ["--build-context", f"build={build}", "-t", tag, str(context)],
        capture_output=True,
        text=True,
    )
    assert built.returncode == 0, built.stderr[-4000:]
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
            assert reply.text == (page(NOINDEX, path) if path.endswith(".html") else text)
        keys = httpx.get(f"{url}/keys", timeout=5)
        assert keys.text == page(NOINDEX)
        assert "microphone=(self)" in keys.headers["permissions-policy"], "the Playground records audio for transcription"
        assert "camera=()" in keys.headers["permissions-policy"]
        assert "media-src 'self' blob:" in keys.headers["content-security-policy"], "generated audio plays from blob: links"
        files = subprocess.run(
            ["docker", "exec", name, "stat", "-c", "%a %U %n", "/etc/caddy/Caddyfile", "/srv", "/srv/index.html", "/srv/assets"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        assert files == ["444 root /etc/caddy/Caddyfile", "755 root /srv", "555 root /srv/index.html", "555 root /srv/assets"]
    finally:
        subprocess.run(["docker", "rm", "-f", name], check=False, capture_output=True)


@contextmanager
def console(image: str, **env: str):
    port = free_port()
    name = f"kaeso-console-robots-{uuid.uuid4().hex[:8]}"
    flags = [arg for key, value in env.items() for arg in ("-e", f"{key}={value}")]
    subprocess.run(["docker", "run", "-d", "--name", name, *flags, "-p", f"127.0.0.1:{port}:3000", image], check=True, capture_output=True)
    try:
        url = f"http://127.0.0.1:{port}"

        def healthy() -> bool:
            try:
                return httpx.get(f"{url}/healthz", timeout=2).text == "ok"
            except httpx.HTTPError:
                return False

        poll(healthy, "the console answering /healthz", seconds=30)
        yield url
    finally:
        subprocess.run(["docker", "rm", "-f", name], check=False, capture_output=True)


def robots_tag(url: str) -> str | None:
    return httpx.get(url, timeout=5).headers.get("x-robots-tag")


def test_by_default_every_answer_of_the_console_asks_crawlers_to_leave_it_out(image):
    with console(image) as url:
        robots = httpx.get(f"{url}/robots.txt", timeout=5)
        assert robots.text == "User-agent: *\nDisallow: /\n"
        assert robots.headers["content-type"] == "text/plain; charset=utf-8"
        for path in ["/", "/sign-in", "/sign-up", "/keys", "/robots.txt", "/healthz", "/favicon.svg", "/assets/index-a1b2c3d4.js", SHARE]:
            assert robots_tag(f"{url}{path}") == NOINDEX, path
        for path in ["/sign-in", "/keys"]:
            assert httpx.get(f"{url}{path}", timeout=5).text == page(NOINDEX), path
        assert httpx.get(f"{url}{SHARE}", timeout=5).text == page(NOINDEX, "share.html")


def test_with_search_engines_on_only_the_sign_in_pages_and_what_draws_them_may_be_indexed(image):
    with console(image, KAESO_SEARCH_ENGINES=" yes ") as url:
        robots = httpx.get(f"{url}/robots.txt", timeout=5)
        assert robots.text.splitlines() == [
            "User-agent: *",
            "Allow: /sign-in$",
            "Allow: /sign-up$",
            "Allow: /assets/",
            "Allow: /theme-init.js",
            "Allow: /favicon",
            "Allow: /apple-touch-icon.png",
            "Disallow: /",
        ]
        for path in ["/sign-in", "/sign-up"]:
            reply = httpx.get(f"{url}{path}", timeout=5)
            assert "x-robots-tag" not in reply.headers, path
            assert reply.text == page(None), path
        for path in ["/favicon.svg", "/assets/index-a1b2c3d4.js"]:
            assert robots_tag(f"{url}{path}") is None, path
        for path in ["/", "/sign-in?next=%2Fkeys", "/keys", "/robots.txt", "/healthz", SHARE]:
            assert robots_tag(f"{url}{path}") == NOINDEX, path
        assert httpx.get(f"{url}/keys", timeout=5).text == page(NOINDEX)
        assert httpx.get(f"{url}{SHARE}", timeout=5).text == page(NOINDEX, "share.html"), "a shared link is never indexed"


def test_the_console_stops_with_a_message_when_kaeso_search_engines_isnt_true_or_false(image):
    run = subprocess.run(["docker", "run", "--rm", "-e", "KAESO_SEARCH_ENGINES=sometimes", image], capture_output=True, text=True, timeout=60)
    assert run.returncode == 1
    assert 'kaeso-console: KAESO_SEARCH_ENGINES must be true or false, as for the gateway, not "sometimes"' in run.stderr


def test_robots_txt_allows_exactly_what_the_console_serves_without_noindex():
    caddyfile = (ROOT / "deploy" / "console" / "Caddyfile").read_text()
    on = caddyfile[caddyfile.index("(search-engines-true) {") :]
    on = on[: on.index("\n}\n")]
    expression = re.search(r"@noindex not expression `(.+)`", on).group(1)
    files_part, pages_part = expression.split(" || ")
    files = re.findall(r"'(/[^']*)'", files_part)
    pages = re.findall(r"'(/[^']*)'", pages_part)
    assert pages_part.endswith("&& {query} == '')"), "a query, such as the sign-in page's next, keeps noindex"
    allowed = [line.strip().removeprefix("Allow: ") for line in on.splitlines() if line.strip().startswith("Allow: ")]
    assert allowed == [f"{p}$" for p in pages] + [f.removesuffix("*") for f in files]
    assert [line.strip() for line in on.splitlines() if line.strip().startswith(("Disallow:", "User-agent:"))] == ["User-agent: *", "Disallow: /"]
    public = {p.name for p in (ROOT / "apps" / "console" / "public").iterdir()}
    vite = (ROOT / "apps" / "console" / "vite.config.ts").read_text()
    for file in files:
        name = file.removeprefix("/")
        if name == "assets/*":
            continue
        shipped = any(p.startswith(name.removesuffix("*")) for p in public) or f'"{name}"' in vite
        assert shipped, f"{file} is a file the console serves"
    assert ROBOTS_META in vite, "the console's build writes the template these tests serve"


def version(text: str) -> tuple[int, ...]:
    return tuple(int(part) for part in text.removeprefix("go").removeprefix("v").split("."))


def test_caddy_is_built_with_the_go_and_x_net_releases_that_fix_the_http2_advisories(image):
    info = subprocess.run(
        ["docker", "run", "--rm", "--entrypoint", "caddy", image, "build-info"], check=True, capture_output=True, text=True
    ).stdout.splitlines()
    fields = [line.split("\t") for line in info]
    go = next(f[1] for f in fields if f[0] == "go")
    net = [f[2] for f in fields if len(f) > 2 and f[0] in ("dep", "=>") and f[1] == "golang.org/x/net"][-1]
    caddy = next(f[2] for f in fields if len(f) > 2 and f[0] == "dep" and f[1] == "github.com/caddyserver/caddy/v2")
    assert version(go) >= (1, 26, 9), go
    assert version(net) >= (0, 60, 0), net
    assert version(caddy) >= (2, 11, 7), caddy


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
