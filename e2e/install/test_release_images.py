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


def test_release_json_names_the_commit_the_attestations_name():
    manifest = job("release-manifest")
    assert '--commit "$GITHUB_SHA"' in manifest
    assert "rev-parse HEAD" not in manifest
    assert "--migrations crates/kaeso-store/migrations" in manifest
    assert "--previous-manifest" in manifest
    assert re.search(r"^    permissions:\n      contents: read\n    env:", manifest, re.M)


def test_only_the_attest_job_can_sign_and_it_sees_nothing_but_release_json():
    attest = job("attest-manifest")
    assert re.search(r"^    permissions:\n      id-token: write\n      attestations: write\n    steps:", attest, re.M)
    assert "actions/checkout" not in attest
    assert "name: release-manifest" in attest
    assert "subject-path: ${{ runner.temp }}/manifest/release.json" in attest
    for other in ["validate", "prepare", "tag", "sync", "public-images", "pack", "release-manifest", "github-release"]:
        assert "id-token: write" not in job(other), other


def test_the_release_is_published_only_when_installs_would_trust_it():
    release = job("github-release")
    assert re.search(r"^    needs: \[[^\]]*\battest-manifest\b[^\]]*\]$", release, re.M)
    assert "attestations: read" in release
    check = release.index("gh attestation verify \"$manifest\"")
    assert '--signer-workflow "$GITHUB_REPOSITORY/.github/workflows/release.yml"' in release
    assert "--source-ref refs/heads/prod" in release
    assert '--source-digest "$commit"' in release
    assert 'gh attestation verify "oci://ghcr.io/logicplanes/kaeso-$image@$digest"' in release
    assert check < release.index("gh release create")


def test_published_digests_stay_long_enough_to_rerun_the_release():
    upload = job("manifest")
    assert re.search(r"name: published-\$\{\{ matrix.image \}\}\n.*?retention-days: 7", upload, re.S)
