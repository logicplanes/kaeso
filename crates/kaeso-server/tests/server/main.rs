mod account_hardening;
mod account_security;
mod accounts;
mod agents;
mod alias_routes;
mod audio;
mod auth;
mod authorization;
mod availability;
mod avatars;
mod caches;
mod catalogue;
mod common;
mod concurrent_changes;
mod configuration_reference;
mod continuity;
mod cors;
mod credentials;
mod custom_providers;
mod dashscope;
mod decisions;
mod deepseek;
mod demo;
mod drills;
mod edge;
mod error_copy;
mod gateway_overhead;
mod gemini_apps;
mod images;
mod instance_users;
mod jobs;
mod key_failures;
mod key_rotation;
mod key_safeguards;
mod keys;
mod limits;
mod logged_secrets;
mod logs;
mod logs_off;
mod mail_language;
mod managed_accounts;
mod mcp_connections;
mod mcp_person_connections;
mod mcp_portal;
mod mcp_servers_api;
mod mcp_tools;
mod metrics_emitted;
mod model_api;
mod notification_dispatch;
mod notification_settings;
mod oidc;
mod operations;
mod org_deletion;
mod org_security;
mod platform_coverage;
mod platform_keys;
mod playground;
mod playground_media;
mod portal_signin;
mod project_people;
mod provider_alibaba;
mod provider_baseten;
mod provider_cerebras;
mod provider_chatgpt;
mod provider_cloudflare_ai_gateway;
mod provider_cohere;
mod provider_deepinfra;
mod provider_errors;
mod provider_fireworks_ai;
mod provider_gemini_agent_platform;
mod provider_google;
mod provider_grok;
mod provider_groq;
mod provider_huggingface;
mod provider_minimax;
mod provider_mistral;
mod provider_moonshotai;
mod provider_nebius;
mod provider_nvidia;
mod provider_ollama_cloud;
mod provider_openrouter;
mod provider_perplexity;
mod provider_sambanova;
mod provider_scaleway;
mod provider_siliconflow;
mod provider_standardcompute;
mod provider_typesafe;
mod provider_upstage;
mod provider_vercel;
mod provider_xai;
mod provider_zai;
mod providers;
mod reads;
mod realtime;
mod reasoning;
mod request_encoding;
mod request_size;
mod response_cache;
mod roles;
mod route_access;
mod route_rights;
mod setup_email;
mod streams;
mod tenancy;
mod time_zone;
mod togetherai;
mod tool_arguments;
mod tool_call_logs;
mod updates;
mod usage;
mod usage_estimates;
mod videos;
mod whats_new;

#[test]
fn every_test_file_is_a_module_of_this_binary() {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/server");
    let main = std::fs::read_to_string(folder.join("main.rs")).unwrap();
    let declared: Vec<&str> = main
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .collect();
    let mut left_out = Vec::new();
    for entry in std::fs::read_dir(&folder).unwrap() {
        let path = entry.unwrap().path();
        let is_module = if path.is_dir() {
            path.join("mod.rs").is_file()
        } else {
            path.extension().is_some_and(|e| e == "rs") && !path.ends_with("main.rs")
        };
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        if is_module && !declared.contains(&name.as_str()) {
            left_out.push(name);
        }
    }
    left_out.sort();
    assert!(
        declared.len() > 100,
        "only {} modules found in main.rs",
        declared.len()
    );
    assert!(
        left_out.is_empty(),
        "these files in tests/server are not compiled, so their tests never run; \
         add `mod <name>;` to tests/server/main.rs: {left_out:?}"
    );
}
