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
    assert '--previous-tag "$previous"' in manifest
    assert re.search(r"^    permissions:\n      contents: read\n    env:", manifest, re.M)


def test_only_the_attest_job_can_sign_and_it_sees_nothing_but_release_json():
    attest = job("attest-manifest")
    assert re.search(r"^    permissions:\n      id-token: write\n      attestations: write\n    steps:", attest, re.M)
    assert "actions/checkout" not in attest
    assert "name: release-manifest" in attest
    assert "subject-path: ${{ runner.temp }}/manifest/release.json" in attest
    for other in ["validate", "notes", "prepare", "versions", "tag", "sync", "public-images", "pack", "release-manifest", "github-release"]:
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


def test_the_published_compose_file_pins_every_image_by_digest_and_matches_release_json():
    manifest = job("release-manifest")
    assert '--compose-out "$RUNNER_TEMP/compose/compose.yaml"' in manifest
    assert re.search(r"name: release-compose\n\s+path: \$\{\{ runner.temp \}\}/compose/compose.yaml", manifest)
    release = job("github-release")
    assert "name: release-compose" in release
    assert "cp deploy/compose.yaml" not in release
    assert 'sha256sum "$compose"' in release
    assert '"    image: ghcr.io/logicplanes/kaeso-$image:$VERSION@$digest"' in release
    assert release.index('sha256sum "$compose"') < release.index("gh release create")


def test_only_the_release_notes_job_sees_the_model_key_and_it_runs_in_the_release_environment():
    notes = job("notes")
    assert re.search(r"^    environment: release$", notes, re.M)
    assert "LOG_AI_KEY: ${{ secrets.LOG_AI_KEY }}" in notes
    assert WORKFLOW.count("secrets.LOG_AI_KEY") == 1


def test_the_validate_job_compares_versions_with_the_workspace_semver():
    validate = job("validate")
    assert "pnpm dlx" not in WORKFLOW
    assert "pnpm exec semver --include-prerelease --range" in validate


def test_the_tag_job_commits_only_what_version_set_writes_on_its_own():
    versions = job("versions")
    assert 'node scripts/version.mjs set "$VERSION"' in versions
    assert "git diff -- . ':(exclude)CHANGELOG.md'" in versions
    assert re.search(r"^    permissions:\n      contents: read\n    env:", versions, re.M)
    tag = job("tag")
    assert re.search(r"^    needs: \[[^\]]*\bversions\b[^\]]*\]$", tag, re.M)
    assert "name: release-versions" in tag
    compare = tag.index('cmp -s "$RUNNER_TEMP/versions/versions.patch" "$RUNNER_TEMP/release/versions.patch"')
    assert compare < tag.index('git commit --message "Release $VERSION"')


def test_the_github_release_job_can_run_again_on_an_existing_release():
    release = job("github-release")
    assert 'gh release view "v$VERSION"' in release
    assert "--clobber" in release
    assert 'gh release edit "v$VERSION"' in release
    assert "--draft=false" in release
    assert release.index('gh release view "v$VERSION"') < release.index('gh release create "v$VERSION"')


def test_the_tag_job_pushes_only_the_tag_so_prod_can_take_updates_from_the_maintainer_alone():
    tag = job("tag")
    assert 'git push origin "refs/tags/v$VERSION"' in tag
    assert "refs/heads/prod" not in tag.split("git fetch --no-tags origin +refs/heads/prod:refs/remotes/origin/prod", 1)[1]
    validate = job("validate")
    assert 'git merge-base --is-ancestor "$latest" HEAD' in validate
    sync = job("sync")
    assert "origin/prod" not in sync
    assert 'v$VERSION^{commit}:refs/heads/dev' in sync


def test_no_release_job_restores_a_cache_another_run_could_have_written():
    assert "cache-from" not in WORKFLOW
    assert "cache-to" not in WORKFLOW
    assert "rust-cache" not in WORKFLOW
    assert "actions/cache" not in WORKFLOW
    assert not re.search(r"^\s+cache: ", WORKFLOW, re.M)
    setups = re.findall(r"uses: actions/setup-node@\w+\n((?:\s{8}.*\n)+)", WORKFLOW)
    assert setups and all("package-manager-cache: false" in setup for setup in setups), setups
    assert re.search(r"^          no-cache: true$", job("image"), re.M)


def test_the_npm_packages_are_packed_from_an_install_without_lifecycle_scripts():
    installs = re.findall(r"pnpm install[^\n]*", job("pack"))
    assert installs == ["pnpm install --frozen-lockfile --ignore-scripts"]


def test_the_crates_compile_without_the_publishing_token_and_upload_unverified():
    build = job("crates-verify")
    assert "id-token" not in build
    assert "environment:" not in build
    assert 'cargo publish --dry-run --locked "${packages[@]}"' in build
    publish = job("crates")
    assert re.search(r"^    needs: \[validate, crates-verify\]$", publish, re.M)
    assert "id-token: write" in publish
    assert re.findall(r"cargo publish[^\n]*", publish) == ['cargo publish --locked --no-verify -p "$crate"']
