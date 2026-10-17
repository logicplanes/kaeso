import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = (ROOT / ".github" / "workflows" / "release.yml").read_text()


def job(name: str) -> str:
    match = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [\w-]+:\n|\Z)", WORKFLOW, re.M | re.S)
    assert match, f"release.yml has no job {name}"
    return match.group(1)


def test_a_release_pulls_every_image_for_both_platforms_without_signing_in():
    pull = job("public-images")
    assert re.search(r"^    needs: \[validate, manifest\]$", pull, re.M)
    assert re.search(r"^    permissions: \{\}$", pull, re.M)
    assert "login" not in pull
    assert 'export DOCKER_CONFIG="$RUNNER_TEMP/anonymous-docker"' in pull
    assert "echo '{}' > \"$DOCKER_CONFIG/config.json\"" in pull
    assert "for image in kaeso-gateway kaeso-console kaeso-updater; do" in pull
    assert 'ref="ghcr.io/logicplanes/$image:$VERSION"' in pull
    assert "for platform in linux/amd64 linux/arm64; do" in pull
    assert 'docker pull --quiet --platform "$platform" "$ref"' in pull
    assert re.search(r"grep -qiE 'denied\|unauthorized\|forbidden\|\\b40\[13\]\\b'", pull)
    assert re.search(r"::error::ghcr\.io refused an anonymous pull .* Make the package public", pull)
    assert 'exit "$failed"' in pull


def test_the_github_release_waits_until_anyone_can_pull_the_images():
    assert re.search(r"^    needs: \[[^\]]*\bpublic-images\b[^\]]*\]$", job("github-release"), re.M)
