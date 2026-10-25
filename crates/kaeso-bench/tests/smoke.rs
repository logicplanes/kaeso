#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::process::Command;

use serde_json::Value;

#[test]
fn every_scenario_runs_at_small_sizes_without_leaks_or_unbounded_memory() {
    let out = std::env::temp_dir().join(format!("kaeso-bench-smoke-{}", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_kaeso-bench"))
        .args(["--smoke", "--out"])
        .arg(&out)
        .status()
        .expect("the benchmark runs");
    let report: Value =
        serde_json::from_slice(&std::fs::read(out.join("report.json")).expect("a JSON report"))
            .unwrap();
    let table = std::fs::read_to_string(out.join("report.md")).unwrap();
    let log = std::fs::read_to_string(out.join("gateway.log")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&out);

    let checks = report["results"]["checks"].as_array().unwrap();
    let failed: Vec<&Value> = checks.iter().filter(|c| c["passed"] != true).collect();
    let warnings: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("\"level\":\"WARN\"") || l.contains("\"level\":\"ERROR\""))
        .collect();
    assert!(
        failed.is_empty(),
        "{failed:#?}\nthe gateway's warnings and errors:\n{}",
        warnings.join("\n")
    );
    assert!(status.success());
    for scenario in [
        "baseline",
        "streams-20",
        "cancellation",
        "slow-clients",
        "mixed",
    ] {
        assert!(
            checks.iter().any(|c| c["scenario"] == scenario),
            "{scenario} ran no check"
        );
    }
    for name in [
        "no upstream stream was left open",
        "every disconnect reached the mock provider",
        "memory stayed bounded",
        "Kaeso logged every stalled stream as client_stalled",
    ] {
        assert!(
            checks.iter().any(|c| c["name"] == name),
            "nothing checked that {name}"
        );
    }
    let results = &report["results"];
    assert_eq!(results["cancellation"]["upstream_left_open"], 0);
    assert_eq!(
        results["cancellation"]["reached_the_mock"],
        results["cancellation"]["cancelled"]
    );
    assert_eq!(
        results["slow_clients"]["counted_as_client_stalled"],
        results["slow_clients"]["stalled"]
    );

    assert_eq!(report["kaeso"]["version"], env!("CARGO_PKG_VERSION"));
    for field in ["cpu", "cores", "memory_bytes"] {
        assert!(!report["machine"][field].is_null(), "{field}");
    }
    assert_eq!(report["parameters"]["smoke"], true);
    assert!(table.contains("| Machine |"), "{table}");
    assert!(table.contains("### Slow clients"), "{table}");
}
