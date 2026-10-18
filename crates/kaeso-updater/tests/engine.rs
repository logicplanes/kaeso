#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use std::path::{Path, PathBuf};

use common::*;
use kaeso_update::Version;
use kaeso_update::manifest::{Image, Service};
use kaeso_update::protocol::{Job, JobState, Reason, Step};
use kaeso_updater::engine::BeginError;
use kaeso_updater::files::{Phase, Saved, StateFile, Switched};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

async fn update(world: &World, to: &str) -> Job {
    world.engine.begin(JOB.into(), v(to)).unwrap();
    world.finished().await
}

fn migrating_releases() -> Releases {
    let mut releases = Releases::standard();
    releases.add(
        TO,
        &Shape {
            postgres: "20261101000000_next",
            ..Shape::default()
        },
    );
    releases
}

#[tokio::test]
async fn an_update_switches_both_services_writes_compose_and_cleans_up() {
    let world = World::new("happy", Daemon::install(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!((job.from.as_str(), job.to.as_str()), (FROM, TO));
    assert_eq!(job.step, Step::Online);
    assert_eq!(job.reason, None);
    assert!(job.finished_at.is_some());
    for service in ["gateway", "console"] {
        let c = world.running(&format!("kaeso-{service}-1"));
        assert_eq!(c.image, format!("sha256:new-{service}"));
        assert!(c.reference.contains("@sha256:"), "{}", c.reference);
        assert_eq!(c.labels["com.docker.compose.project"], "kaeso");
        assert!(c.env.contains(&"KAESO_DOMAIN=kaeso.example.com".to_owned()));
    }
    assert_eq!(
        world.names(),
        ["kaeso-console-1", "kaeso-gateway-1", "kaeso-updater-1"]
    );
    assert_eq!(world.compose(), compose_text(TO));
    assert!(!world.maintenance());
    assert_eq!(world.calls("POST images/create").len(), 3);
    assert_eq!(world.calls("POST images/").len() - 3, 3, "three tags");
    let seen = world.verifier.seen.lock().unwrap().clone();
    assert_eq!(
        seen,
        [
            "release.json",
            Service::Gateway.image(),
            Service::Console.image(),
            Service::Updater.image(),
            "release.json",
        ]
    );
    let stops: Vec<String> = world
        .calls("POST containers/")
        .into_iter()
        .filter(|c| c.ends_with("/stop"))
        .collect();
    assert_eq!(stops.len(), 2);
    assert!(
        stops[0].contains("consoleold"),
        "the console switches first: {stops:?}"
    );
    let saved = StateFile::new(world.dir.join("state")).load().unwrap();
    assert_eq!(saved.job, job);
    assert!(!saved.migrates);
    assert_eq!(saved.images.len(), 3);
    assert_eq!(
        StateFile::new(world.dir.join("state")).kept_compose(),
        Some(compose_text(FROM))
    );
    let created = world.daemon.lock().unwrap().created.clone();
    let gateway = created
        .iter()
        .find(|s| s["Labels"]["com.docker.compose.service"] == "gateway")
        .unwrap();
    assert_eq!(
        gateway["Healthcheck"]["StartPeriod"], 400_000_000u64,
        "the health check waits as long as the migrations may take"
    );
    let console = created
        .iter()
        .find(|s| s["Labels"]["com.docker.compose.service"] == "console")
        .unwrap();
    assert!(console.get("Healthcheck").is_none());
}

#[tokio::test]
async fn a_gateway_unhealthy_while_it_migrates_is_waited_for() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Slow);
    let world = World::new("slow", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:new-gateway");
}

#[tokio::test]
async fn a_gateway_that_stays_unhealthy_rolls_back_when_nothing_migrates() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("gateway".into(), Health::Unhealthy);
    let world = World::new("unhealthy", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(job.reason, Some(Reason::Unhealthy));
    assert_eq!(job.step, Step::Migrating);
    assert!(
        job.detail
            .unwrap()
            .contains("the new gateway was not healthy within")
    );
    for service in ["gateway", "console"] {
        let c = world.running(&format!("kaeso-{service}-1"));
        assert_eq!(c.image, format!("sha256:old-{service}"));
    }
    assert_eq!(
        world.names(),
        ["kaeso-console-1", "kaeso-gateway-1", "kaeso-updater-1"]
    );
    assert_eq!(world.compose(), compose_text(FROM));
    assert!(!world.maintenance());
}

#[tokio::test]
async fn a_gateway_that_crashes_or_restarts_is_rolled_back_at_once() {
    for health in [Health::Crash, Health::Restarting] {
        let mut daemon = Daemon::install();
        daemon.new_health.insert("gateway".into(), health);
        let world = World::new(&format!("{health:?}"), daemon, Verifier::default()).await;
        let job = update(&world, TO).await;
        assert_eq!(job.state, JobState::RolledBack, "{job:?}");
        let detail = job.detail.unwrap();
        assert!(
            detail.contains("restarted after a crash")
                || detail.contains("stopped with exit code 1"),
            "{detail}"
        );
    }
}

#[tokio::test]
async fn a_failed_gateway_that_may_have_migrated_is_left_for_a_person() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Crash);
    let dir = fresh("needs-attention");
    let world = World::with(dir, daemon, migrating_releases(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
    assert_eq!(job.reason, Some(Reason::Migrated));
    let detail = job.detail.unwrap();
    assert!(
        detail.contains("may have migrated the database"),
        "{detail}"
    );
    assert!(detail.contains("kaeso-gateway-1-previous"), "{detail}");
    assert!(detail.contains("kaeso-console-1-previous"), "{detail}");
    assert_eq!(
        world.running("kaeso-console-1").image,
        "sha256:new-console",
        "the console stays with the gateway whose API it speaks"
    );
    assert_eq!(
        world.container("kaeso-gateway-1").unwrap().image,
        "sha256:new-gateway"
    );
    for service in ["gateway", "console"] {
        let previous = world
            .container(&format!("kaeso-{service}-1-previous"))
            .unwrap();
        assert!(
            !previous.running,
            "the previous {service} is not started again"
        );
    }
    assert_eq!(world.compose(), compose_text(FROM));
    assert!(!world.maintenance());
}

#[tokio::test]
async fn a_gateway_docker_started_though_the_start_call_failed_counts_as_started() {
    let mut daemon = Daemon::install();
    daemon.fail_new_start = Some("gateway".into());
    let world = World::with(
        fresh("start-broke"),
        daemon,
        migrating_releases(),
        Verifier::default(),
    )
    .await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
    assert_eq!(
        world.container("kaeso-gateway-1").unwrap().image,
        "sha256:new-gateway"
    );

    let mut daemon = Daemon::install();
    daemon.fail_new_start = Some("gateway".into());
    let world = World::new("start-broke-same", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(
        job.state,
        JobState::RolledBack,
        "without new migrations it rolls back: {job:?}"
    );
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:old-gateway");
}

#[tokio::test]
async fn a_running_release_without_a_manifest_counts_as_migrating() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Crash);
    let mut releases = Releases::standard();
    releases.by_version.remove(FROM);
    let world = World::with(fresh("no-from"), daemon, releases, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
}

#[tokio::test]
async fn a_console_that_fails_is_rolled_back_before_the_gateway_is_touched() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("console".into(), Health::Unhealthy);
    let world = World::with(
        fresh("console"),
        daemon,
        migrating_releases(),
        Verifier::default(),
    )
    .await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(job.step, Step::Switching);
    assert!(world.calls("POST containers/gatewayold").is_empty());
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:old-gateway");
}

#[tokio::test]
async fn a_previous_version_that_does_not_come_back_is_reported() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("gateway".into(), Health::Unhealthy);
    daemon.old_health_after_restart = Some(Health::Crash);
    let world = World::new("rollback-failed", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RollbackFailed, "{job:?}");
    assert!(job.detail.unwrap().contains("rolling back failed"));
    assert!(!world.maintenance());
}

#[tokio::test]
async fn refusals_before_anything_changes() {
    type Change = Box<dyn Fn(&mut Daemon)>;
    let cases: Vec<(&str, &str, Change, Reason, &str)> = vec![
        ("major", "41.0.0", Box::new(|_| {}), Reason::Major, "major"),
        (
            "manual",
            "40.3.0",
            Box::new(|_| {}),
            Reason::ManualSteps,
            "(40.3.0)",
        ),
        (
            "crossed",
            "40.2.2",
            Box::new(|_| {}),
            Reason::ManualSteps,
            "(40.2.1)",
        ),
        ("same", FROM, Box::new(|_| {}), Reason::NotNewer, "runs"),
        (
            "missing",
            "40.9.9",
            Box::new(|_| {}),
            Reason::ManifestUnavailable,
            "40.9.9",
        ),
        (
            "pull",
            TO,
            Box::new(|d| d.fail_pull = true),
            Reason::PullFailed,
            "denied",
        ),
        (
            "label",
            TO,
            Box::new(|d| d.pulled_version = Some("40.1.9".into())),
            Reason::Unverified,
            "says it is 40.1.9",
        ),
        (
            "two",
            TO,
            Box::new(|d| {
                let mut extra = d.container("kaeso-console-1").unwrap().clone();
                extra.id = format!("{:0<64}", "consoletwo");
                extra.name = "kaeso-console-2".into();
                d.containers.push(extra);
            }),
            Reason::ServicesNotFound,
            "(kaeso-console-1, kaeso-console-2); remove all but the one Compose started",
        ),
        (
            "stale",
            TO,
            Box::new(|d| {
                let mut stale = d.container("kaeso-gateway-1").unwrap().clone();
                stale.id = format!("{:0<64}", "gatewaystale");
                stale.name = "kaeso-gateway-1-previous".into();
                stale.running = false;
                d.containers.push(stale);
            }),
            Reason::ServicesNotFound,
            "`docker rm kaeso-gateway-1-previous`",
        ),
    ];
    for (name, to, change, reason, says) in cases {
        let mut daemon = Daemon::install();
        change(&mut daemon);
        let world = World::new(name, daemon, Verifier::default()).await;
        let job = update(&world, to).await;
        assert_eq!(job.state, JobState::Failed, "{name}: {job:?}");
        assert_eq!(job.reason, Some(reason), "{name}: {job:?}");
        assert!(
            job.detail.as_deref().unwrap_or_default().contains(says),
            "{name}: {job:?}"
        );
        assert!(world.calls("POST containers/").is_empty(), "{name}");
        assert_eq!(world.compose(), compose_text(FROM), "{name}");
        assert!(!world.maintenance(), "{name}");
    }
}

#[tokio::test]
async fn one_off_containers_of_a_service_are_not_the_service() {
    let mut daemon = Daemon::install();
    let mut run = daemon.container("kaeso-gateway-1").unwrap().clone();
    run.id = format!("{:0<64}", "gatewayrun");
    run.name = "kaeso-gateway-run-1a2b3c".into();
    run.labels
        .insert("com.docker.compose.oneoff".into(), "True".into());
    daemon.containers.push(run);
    let world = World::new("oneoff", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
}

#[tokio::test]
async fn an_image_without_the_release_workflows_signature_is_never_pulled() {
    for rejected in ["release.json", Service::Console.image()] {
        let verifier = Verifier {
            reject: Some(rejected.into()),
            ..Verifier::default()
        };
        let world = World::new("unverified", Daemon::install(), verifier).await;
        let job = update(&world, TO).await;
        assert_eq!(job.state, JobState::Failed);
        assert_eq!(job.reason, Some(Reason::Unverified));
        assert!(job.detail.unwrap().contains(rejected));
        assert!(world.calls("POST images/create").is_empty(), "{rejected}");
        assert!(world.calls("POST containers/").is_empty());
    }
}

#[tokio::test]
async fn a_changed_compose_file_is_never_overwritten() {
    let world = World::new("compose", Daemon::install(), Verifier::default()).await;
    let mut local = compose_text(FROM);
    local.extend_from_slice(b"  extra:\n    image: example\n");
    std::fs::write(world.dir.join("compose.yaml"), &local).unwrap();
    let job = update(&world, TO).await;
    assert_eq!(job.reason, Some(Reason::ComposeChanged));
    assert_eq!(world.compose(), local);
    assert!(world.calls("POST images/create").is_empty());
}

fn on_start(daemon: &mut Daemon, dir: &Path, f: fn(&Path)) {
    let dir = dir.to_path_buf();
    daemon.on_start = Some(Box::new(move || f(&dir)));
}

#[tokio::test]
async fn compose_changed_during_the_update_is_left_alone_and_said() {
    let dir = fresh("compose-during");
    let mut daemon = Daemon::install();
    on_start(&mut daemon, &dir, |dir| {
        let mut edited = compose_text(FROM);
        edited.extend_from_slice(b"  extra:\n    image: example\n");
        std::fs::write(dir.join("compose.yaml"), edited).unwrap();
    });
    let world = World::with(dir, daemon, Releases::standard(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(job.reason, Some(Reason::ComposeChanged));
    assert!(job.detail.unwrap().contains("changed during the update"));
    assert!(world.compose().ends_with(b"image: example\n"));
}

#[tokio::test]
async fn progress_that_cannot_be_saved_stops_before_the_next_container_is_touched() {
    let dir = fresh("state-lost");
    let mut daemon = Daemon::install();
    on_start(&mut daemon, &dir, |dir| {
        std::fs::remove_dir_all(dir.join("state")).unwrap();
    });
    let world = World::with(dir, daemon, Releases::standard(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(job.reason, Some(Reason::StateUnwritable));
    assert!(
        world
            .calls("POST containers/")
            .iter()
            .all(|c| !c.contains("gatewayold")),
        "the gateway is never stopped"
    );
    assert_eq!(world.running("kaeso-console-1").image, "sha256:old-console");
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:old-gateway");
}

#[tokio::test]
async fn a_previous_container_that_restarted_long_ago_still_comes_back() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("gateway".into(), Health::Unhealthy);
    for c in daemon.containers.iter_mut() {
        c.restarts = 2;
    }
    let world = World::new("old-restarts", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:old-gateway");
}

#[tokio::test]
async fn a_previous_container_that_keeps_its_temporary_name_is_a_failed_rollback() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("gateway".into(), Health::Unhealthy);
    daemon.fail_rename_back = true;
    let world = World::new("rename-back", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::RollbackFailed, "{job:?}");
    assert!(
        job.detail
            .unwrap()
            .contains("keeps its name kaeso-gateway-1-previous")
    );
}

#[tokio::test]
async fn previous_containers_that_cannot_be_removed_are_left_for_a_person() {
    let mut daemon = Daemon::install();
    daemon.fail_removals = 2;
    let world = World::new("cleanup-retry", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(
        job.state,
        JobState::Succeeded,
        "a retry removes it: {job:?}"
    );

    let mut daemon = Daemon::install();
    daemon.fail_removals = u32::MAX;
    let world = World::new("cleanup-stuck", daemon, Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
    let detail = job.detail.unwrap();
    assert!(
        detail.contains("docker rm kaeso-console-1-previous kaeso-gateway-1-previous"),
        "{detail}"
    );
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:new-gateway");
}

#[tokio::test]
async fn no_container_is_touched_when_the_maintenance_flag_cannot_be_written() {
    let world = World::new("flag-unwritable", Daemon::install(), Verifier::default()).await;
    std::fs::remove_dir_all(world.dir.join("maintenance")).unwrap();
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Failed, "{job:?}");
    assert_eq!(job.reason, Some(Reason::StateUnwritable));
    assert!(world.calls("POST containers/").is_empty());
    assert_eq!(world.compose(), compose_text(FROM));
}

#[tokio::test]
async fn a_maintenance_flag_that_stays_is_left_for_a_person() {
    let dir = fresh("flag-stuck");
    let mut daemon = Daemon::install();
    on_start(&mut daemon, &dir, |dir| {
        let flag = dir.join("maintenance/active");
        let _ = std::fs::remove_file(&flag);
        std::fs::create_dir_all(flag.join("stuck")).unwrap();
    });
    let world = World::with(dir, daemon, Releases::standard(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
    let detail = job.detail.unwrap();
    assert!(
        detail.contains("the maintenance flag could not be removed"),
        "{detail}"
    );
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:new-gateway");
}

#[cfg(unix)]
#[tokio::test]
async fn a_compose_file_that_cannot_be_written_at_the_end_is_a_warning() {
    use std::os::unix::fs::PermissionsExt;
    let dir = fresh("compose-readonly");
    let mut daemon = Daemon::install();
    on_start(&mut daemon, &dir, |dir| {
        std::fs::set_permissions(
            dir.join("compose.yaml"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();
    });
    let world = World::with(dir, daemon, Releases::standard(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(job.reason, Some(Reason::ComposeUnwritable));
    assert!(job.detail.unwrap().contains("by hand"));
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:new-gateway");
}

#[tokio::test]
async fn a_second_update_while_one_runs_is_refused() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Starting);
    let world = World::new("busy", daemon, Verifier::default()).await;
    world.engine.begin(JOB.into(), v(TO)).unwrap();
    assert!(matches!(
        world
            .engine
            .begin("upd_01k7a3f8m2xq4v9c5r6t8w0yz2".into(), v(TO)),
        Err(BeginError::Busy)
    ));
    assert_eq!(world.engine.job(JOB).unwrap().state, JobState::Running);
    assert!(world.engine.job("upd_01k7a3f8m2xq4v9c5r6t8w0yz2").is_none());
    world.finished().await;
    assert!(
        world
            .engine
            .begin("upd_01k7a3f8m2xq4v9c5r6t8w0yz3".into(), v(TO))
            .is_ok()
    );
    world.finished().await;
}

#[tokio::test]
async fn an_update_whose_state_cannot_be_saved_never_starts() {
    let world = World::new("no-state", Daemon::install(), Verifier::default()).await;
    std::fs::remove_dir_all(world.dir.join("state")).unwrap();
    let err = world.engine.begin(JOB.into(), v(TO)).unwrap_err();
    assert!(matches!(err, BeginError::State(_)), "{err}");
    assert_eq!(world.engine.last(), None);
    assert!(world.calls("").is_empty());
}

fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kaeso-updater-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["maintenance", "state"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::fs::write(dir.join("compose.yaml"), compose_text(FROM)).unwrap();
    dir
}

fn interrupted(step: Step, switched: Vec<Switched>) -> Saved {
    let mut saved = Saved::new(Job {
        id: JOB.into(),
        from: FROM.into(),
        to: TO.into(),
        state: JobState::Running,
        step,
        reason: None,
        detail: None,
        started_at: time::OffsetDateTime::UNIX_EPOCH,
        finished_at: None,
    });
    saved.switched = switched;
    saved.project = Some("kaeso".into());
    saved.migrates = false;
    saved
}

fn previous(daemon: &mut Daemon, service: &str) -> String {
    let old = daemon
        .containers
        .iter_mut()
        .find(|c| c.name == format!("kaeso-{service}-1"))
        .unwrap();
    old.name = format!("kaeso-{service}-1-previous");
    old.running = false;
    old.id.clone()
}

fn switched(service: Service, old_id: String, new_id: Option<&str>, phase: Phase) -> Switched {
    Switched {
        service,
        name: format!("kaeso-{}-1", service.as_str()),
        old_id,
        new_id: new_id.map(str::to_owned),
        phase,
    }
}

#[tokio::test]
async fn an_update_interrupted_while_switching_is_rolled_back_at_start() {
    let mut daemon = Daemon::install();
    let old_id = previous(&mut daemon, "gateway");
    let dir = fresh("recover");
    std::fs::write(dir.join("maintenance/active"), b"{}").unwrap();
    StateFile::new(dir.join("state"))
        .save(&interrupted(
            Step::Migrating,
            vec![switched(Service::Gateway, old_id, None, Phase::Stopping)],
        ))
        .unwrap();
    let world = World::with(dir, daemon, Releases::default(), Verifier::default()).await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(job.reason, Some(Reason::Interrupted));
    assert_eq!(world.running("kaeso-gateway-1").image, "sha256:old-gateway");
    assert!(!world.maintenance());
}

#[tokio::test]
async fn a_container_created_before_the_updater_saved_it_is_removed_on_rollback() {
    let mut daemon = Daemon::install();
    let old_id = previous(&mut daemon, "gateway");
    let mut stray = daemon.container("kaeso-console-1").unwrap().clone();
    stray.id = format!("{:0<64}", "new99");
    stray.name = "kaeso-gateway-1".into();
    stray
        .labels
        .insert("com.docker.compose.service".into(), "gateway".into());
    stray.running = false;
    daemon.containers.push(stray);
    let dir = fresh("stray");
    StateFile::new(dir.join("state"))
        .save(&interrupted(
            Step::Migrating,
            vec![switched(
                Service::Gateway,
                old_id.clone(),
                None,
                Phase::Creating,
            )],
        ))
        .unwrap();
    let world = World::with(dir, daemon, Releases::default(), Verifier::default()).await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    let gateway = world.running("kaeso-gateway-1");
    assert_eq!(gateway.id, old_id);
    assert_eq!(
        world.names(),
        ["kaeso-console-1", "kaeso-gateway-1", "kaeso-updater-1"]
    );
}

#[tokio::test]
async fn an_update_interrupted_while_the_gateway_migrates_finishes_when_it_turns_healthy() {
    for phase in [Phase::Started, Phase::Created] {
        interrupted_while_migrating(phase).await;
    }
}

async fn interrupted_while_migrating(phase: Phase) {
    let mut daemon = Daemon::install();
    let old_id = previous(&mut daemon, "gateway");
    let mut new = daemon.container("kaeso-console-1").unwrap().clone();
    new.id = format!("{:0<64}", "new7");
    new.name = "kaeso-gateway-1".into();
    new.image = "sha256:new-gateway".into();
    new.labels
        .insert("com.docker.compose.service".into(), "gateway".into());
    new.health = Health::Slow;
    new.started = true;
    daemon.containers.push(new);
    let dir = fresh(&format!("recover-forward-{phase:?}"));
    let state = StateFile::new(dir.join("state"));
    state.keep_compose(&compose_text(FROM)).unwrap();
    state
        .save(&interrupted(
            Step::Migrating,
            vec![switched(
                Service::Gateway,
                old_id,
                Some(&format!("{:0<64}", "new7")),
                phase,
            )],
        ))
        .unwrap();
    let world = World::with(dir, daemon, Releases::default(), Verifier::default()).await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(world.compose(), compose_text(TO));
    assert!(world.container("kaeso-gateway-1-previous").is_none());
}

#[tokio::test]
async fn an_update_interrupted_when_online_is_finished_and_tagged_at_start() {
    let dir = fresh("online");
    let torn = compose_text(FROM)[..20].to_vec();
    std::fs::write(dir.join("compose.yaml"), &torn).unwrap();
    let state = StateFile::new(dir.join("state"));
    state.keep_compose(&compose_text(FROM)).unwrap();
    let mut saved = interrupted(Step::Online, Vec::new());
    saved.images = [Service::Gateway, Service::Console, Service::Updater]
        .iter()
        .map(|s| Image {
            name: s.image().into(),
            digest: digest_of(*s),
        })
        .collect();
    state.save(&saved).unwrap();
    let world = World::with(
        dir,
        Daemon::install(),
        Releases::default(),
        Verifier::default(),
    )
    .await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(job.reason, None);
    assert_eq!(
        world.compose(),
        compose_text(TO),
        "a torn compose.yaml is restored"
    );
    let tags: Vec<String> = world
        .calls("POST images/")
        .into_iter()
        .filter(|c| c.ends_with("/tag"))
        .collect();
    assert_eq!(tags.len(), 3, "{tags:?}");
}

#[tokio::test]
async fn compose_yaml_edited_before_a_restart_is_left_alone() {
    let dir = fresh("online-edited");
    let mut edited = compose_text(FROM);
    edited.extend_from_slice(b"  extra:\n    image: example\n");
    std::fs::write(dir.join("compose.yaml"), &edited).unwrap();
    let state = StateFile::new(dir.join("state"));
    state.keep_compose(&compose_text(FROM)).unwrap();
    state.save(&interrupted(Step::Online, Vec::new())).unwrap();
    let world = World::with(
        dir,
        Daemon::install(),
        Releases::default(),
        Verifier::default(),
    )
    .await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!(job.reason, Some(Reason::ComposeChanged));
    assert_eq!(world.compose(), edited);
}

#[tokio::test]
async fn an_update_interrupted_before_switching_just_failed() {
    let dir = fresh("early");
    StateFile::new(dir.join("state"))
        .save(&interrupted(Step::Verifying, Vec::new()))
        .unwrap();
    let world = World::with(
        dir,
        Daemon::install(),
        Releases::default(),
        Verifier::default(),
    )
    .await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(
        (job.state, job.reason),
        (JobState::Failed, Some(Reason::Interrupted))
    );
    world.engine.recover().await;
    assert_eq!(world.engine.last().unwrap(), job);
}

#[tokio::test]
async fn recovery_waits_for_docker_to_answer() {
    let mut daemon = Daemon::install();
    let old_id = previous(&mut daemon, "gateway");
    daemon.version_failures = 5;
    let dir = fresh("docker-late");
    StateFile::new(dir.join("state"))
        .save(&interrupted(
            Step::Switching,
            vec![switched(Service::Gateway, old_id, None, Phase::Stopping)],
        ))
        .unwrap();
    let world = World::with(dir, daemon, Releases::default(), Verifier::default()).await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::RolledBack, "{job:?}");
    assert_eq!(world.calls("GET version").len(), 6);
}

#[tokio::test]
async fn recovery_without_docker_hands_over_to_a_person() {
    let mut daemon = Daemon::install();
    let old_id = previous(&mut daemon, "gateway");
    daemon.version_failures = u32::MAX;
    let dir = fresh("docker-gone");
    StateFile::new(dir.join("state"))
        .save(&interrupted(
            Step::Switching,
            vec![switched(Service::Gateway, old_id, None, Phase::Stopping)],
        ))
        .unwrap();
    let world = World::with(dir, daemon, Releases::default(), Verifier::default()).await;
    world.engine.recover().await;
    let job = world.engine.last().unwrap();
    assert_eq!(job.state, JobState::NeedsAttention, "{job:?}");
    assert_eq!(job.reason, Some(Reason::DockerUnavailable));
    assert!(world.calls("POST").is_empty());
}

#[tokio::test]
async fn the_status_reports_docker_and_the_last_job() {
    let world = World::new("status", Daemon::install(), Verifier::default()).await;
    let status = world.engine.status().await;
    assert!(status.docker);
    assert_eq!(status.protocol, kaeso_update::protocol::VERSION);
    assert_eq!(status.job, None);
}
