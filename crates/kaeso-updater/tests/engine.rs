#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::*;
use kaeso_update::Version;
use kaeso_update::manifest::Service;
use kaeso_update::protocol::{Job, JobState, Reason, Step};
use kaeso_updater::files::{Saved, StateFile, Switched};

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

async fn update(world: &World, to: &str) -> Job {
    world.engine.begin(JOB.into(), v(to)).unwrap();
    world.finished().await
}

#[tokio::test]
async fn an_update_switches_both_services_writes_compose_and_cleans_up() {
    let world = World::new("happy", Daemon::install(), Verifier::default()).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Succeeded, "{job:?}");
    assert_eq!((job.from.as_str(), job.to.as_str()), (FROM, TO));
    assert_eq!(job.step, Step::Online);
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
            Service::Updater.image()
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
}

#[tokio::test]
async fn an_unhealthy_gateway_rolls_back_to_the_previous_containers() {
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
            .contains("the new gateway reported itself unhealthy")
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
async fn a_gateway_that_never_gets_healthy_or_crashes_is_rolled_back() {
    for health in [Health::Starting, Health::Crash] {
        let mut daemon = Daemon::install();
        daemon.new_health.insert("gateway".into(), health);
        let world = World::new(&format!("{health:?}"), daemon, Verifier::default()).await;
        let job = update(&world, TO).await;
        assert_eq!(job.state, JobState::RolledBack, "{job:?}");
        let detail = job.detail.unwrap();
        assert!(
            detail.contains("was not healthy within")
                || detail.contains("stopped with exit code 1"),
            "{detail}"
        );
    }
}

#[tokio::test]
async fn a_console_that_fails_is_rolled_back_before_the_gateway_is_touched() {
    let mut daemon = Daemon::install();
    daemon
        .new_health
        .insert("console".into(), Health::Unhealthy);
    let world = World::new("console", daemon, Verifier::default()).await;
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
    let cases: Vec<(&str, &str, Change, Reason)> = vec![
        ("major", "1.0.0", Box::new(|_| {}), Reason::Major),
        ("manual", "0.3.0", Box::new(|_| {}), Reason::ManualSteps),
        ("same", FROM, Box::new(|_| {}), Reason::NotNewer),
        (
            "missing",
            "0.9.9",
            Box::new(|_| {}),
            Reason::ManifestUnavailable,
        ),
        (
            "pull",
            TO,
            Box::new(|d| d.fail_pull = true),
            Reason::PullFailed,
        ),
        (
            "label",
            TO,
            Box::new(|d| d.pulled_version = Some("0.1.9".into())),
            Reason::Unverified,
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
        ),
    ];
    for (name, to, change, reason) in cases {
        let mut daemon = Daemon::install();
        change(&mut daemon);
        let world = World::new(name, daemon, Verifier::default()).await;
        let job = update(&world, to).await;
        assert_eq!(job.state, JobState::Failed, "{name}: {job:?}");
        assert_eq!(job.reason, Some(reason), "{name}: {job:?}");
        assert!(world.calls("POST containers/").is_empty(), "{name}");
        assert_eq!(world.compose(), compose_text(FROM), "{name}");
        assert!(!world.maintenance(), "{name}");
    }
}

#[tokio::test]
async fn an_image_without_the_release_workflows_signature_is_never_switched_to() {
    let verifier = Verifier {
        reject: Some(Service::Console.image().into()),
        ..Verifier::default()
    };
    let world = World::new("unverified", Daemon::install(), verifier).await;
    let job = update(&world, TO).await;
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.reason, Some(Reason::Unverified));
    assert!(job.detail.unwrap().contains("kaeso-console"));
    assert!(world.calls("POST containers/").is_empty());
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

#[tokio::test]
async fn a_second_update_while_one_runs_is_refused() {
    let mut daemon = Daemon::install();
    daemon.new_health.insert("gateway".into(), Health::Starting);
    let world = World::new("busy", daemon, Verifier::default()).await;
    world.engine.begin(JOB.into(), v(TO)).unwrap();
    assert!(
        world
            .engine
            .begin("upd_01k7a3f8m2xq4v9c5r6t8w0yz2".into(), v(TO))
            .is_err()
    );
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

fn interrupted(step: Step, switched: Vec<Switched>) -> Saved {
    Saved {
        job: Job {
            id: JOB.into(),
            from: FROM.into(),
            to: TO.into(),
            state: JobState::Running,
            step,
            reason: None,
            detail: None,
            started_at: time::OffsetDateTime::UNIX_EPOCH,
            finished_at: None,
        },
        switched,
    }
}

#[tokio::test]
async fn an_update_interrupted_while_switching_is_rolled_back_at_start() {
    let mut daemon = Daemon::install();
    let old = daemon
        .containers
        .iter_mut()
        .find(|c| c.name == "kaeso-gateway-1")
        .unwrap();
    old.name = "kaeso-gateway-1-previous".into();
    old.running = false;
    let old_id = old.id.clone();
    let dir = std::env::temp_dir().join(format!("kaeso-updater-recover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["maintenance", "state"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::fs::write(dir.join("compose.yaml"), compose_text(FROM)).unwrap();
    std::fs::write(dir.join("maintenance/active"), b"{}").unwrap();
    StateFile::new(dir.join("state"))
        .save(&interrupted(
            Step::Migrating,
            vec![Switched {
                service: Service::Gateway,
                name: "kaeso-gateway-1".into(),
                old_id,
                new_id: None,
            }],
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
async fn an_update_interrupted_when_online_is_finished_at_start() {
    let dir = std::env::temp_dir().join(format!("kaeso-updater-online-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["maintenance", "state"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::fs::write(dir.join("compose.yaml"), compose_text(FROM)).unwrap();
    StateFile::new(dir.join("state"))
        .save(&interrupted(Step::Online, Vec::new()))
        .unwrap();
    let world = World::with(
        dir,
        Daemon::install(),
        Releases::default(),
        Verifier::default(),
    )
    .await;
    world.engine.recover().await;
    assert_eq!(world.engine.last().unwrap().state, JobState::Succeeded);
    assert_eq!(world.compose(), compose_text(TO));
}

#[tokio::test]
async fn an_update_interrupted_before_switching_just_failed() {
    let dir = std::env::temp_dir().join(format!("kaeso-updater-early-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["maintenance", "state"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    std::fs::write(dir.join("compose.yaml"), compose_text(FROM)).unwrap();
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
async fn the_status_reports_docker_and_the_last_job() {
    let world = World::new("status", Daemon::install(), Verifier::default()).await;
    let status = world.engine.status().await;
    assert!(status.docker);
    assert_eq!(status.protocol, kaeso_update::protocol::VERSION);
    assert_eq!(status.job, None);
}
