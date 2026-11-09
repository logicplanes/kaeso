use kaeso_codecs::anthropic_messages::MessagesProfile;
use kaeso_codecs::openai_chat::ChatProfile;
use kaeso_codecs::openai_responses::ResponsesProfile;
use serde::{Deserialize, Serialize};

use crate::providers::MediaApi;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "openapi",
    schema(description = "The inference server software a custom provider runs.")
)]
pub enum ServerKind {
    #[doc = "vLLM."]
    Vllm,
    #[doc = "SGLang."]
    Sglang,
    #[doc = "Ollama."]
    Ollama,
    #[doc = "LM Studio."]
    LmStudio,
    #[doc = "llama.cpp's `llama-server`, also inside llamafile."]
    LlamaCpp,
    #[doc = "LocalAI."]
    Localai,
    #[doc = "Hugging Face Text Generation Inference."]
    Tgi,
    #[doc = "LMDeploy."]
    Lmdeploy,
    #[doc = "NVIDIA NIM for LLMs."]
    Nim,
    #[doc = "Docker Model Runner."]
    DockerModelRunner,
    #[doc = "KoboldCpp."]
    Koboldcpp,
    #[doc = "TabbyAPI."]
    Tabbyapi,
    #[doc = "Xorbits Inference (Xinference)."]
    Xinference,
    #[doc = "MLX LM's `mlx_lm.server`."]
    MlxLm,
    #[doc = "Jan's local API server."]
    Jan,
}

// A native endpoint that answers in a shape only one server software has, asked with a GET
// that costs nothing when the model listing doesn't tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub path: &'static str,
    pub answer: Answer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    // An object whose only member is a string under this name.
    OnlyString(&'static str),
    // An object with this string member set to this value.
    Member(&'static str, &'static str),
}

impl Answer {
    pub fn matches(self, body: &[u8]) -> bool {
        let Ok(serde_json::Value::Object(map)) = serde_json::from_slice(body) else {
            return false;
        };
        match self {
            Answer::OnlyString(name) => {
                map.len() == 1 && map.get(name).is_some_and(|v| v.is_string())
            }
            Answer::Member(name, value) => map.get(name).and_then(|v| v.as_str()) == Some(value),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Server {
    pub kind: ServerKind,
    pub name: &'static str,
    pub suggested_id: &'static str,
    pub port: u16,
    // What comes between the address and `/v1`, such as Docker Model Runner's `/engines`.
    pub path: &'static str,
    pub owned_by: &'static [&'static str],
    pub server_header: Option<&'static str>,
    pub probe: Option<Probe>,
    pub chat: ChatProfile,
    pub responses: ResponsesProfile,
    pub messages: MessagesProfile,
    pub context_window_messages: &'static [&'static str],
    /// OpenAI's media APIs the server software documents, which a new custom provider for it
    /// serves unless its admin says otherwise.
    pub media: &'static [MediaApi],
}

// Every server here renders the conversation through the model's chat template, which knows
// `system` but often not `developer`: Qwen's template drops a `developer` message without a
// word. Their own prefix caching needs no `prompt_cache_key`, and no end-user id reaches them.
const fn local(name: &'static str) -> ChatProfile {
    ChatProfile::compatible(name)
        .system_role()
        .max_tokens()
        .without_end_user()
        .without_cache_key()
}

// https://github.com/vllm-project/vllm/blob/c3a347a548/vllm/renderers/params.py#L405
// https://github.com/vllm-project/vllm/blob/c3a347a548/vllm/v1/engine/input_processor.py#L585
const VLLM_CONTEXT: &[&str] = &[
    "maximum context length is",
    "longer than the maximum model length",
];
// https://github.com/ggml-org/llama.cpp/blob/7d7f028753/tools/server/server-context.cpp
const LLAMA_CPP_CONTEXT: &[&str] = &["exceeds the available context size"];

const VLLM: Server = Server {
    kind: ServerKind::Vllm,
    name: "vLLM",
    suggested_id: "vllm",
    port: 8000,
    path: "",
    owned_by: &["vllm"],
    server_header: None,
    probe: None,
    // vLLM sends thinking as `reasoning` since 0.11.1 and as `reasoning_content` before.
    chat: local("vLLM")
        .reasoning_fields(&["reasoning", "reasoning_content"])
        .with_video_url(),
    responses: ResponsesProfile::compatible("vLLM"),
    messages: MessagesProfile::compatible("vLLM"),
    context_window_messages: VLLM_CONTEXT,
    // https://docs.vllm.ai/en/latest/serving/openai_compatible_server.html, its Transcriptions
    // and Translations APIs, read 2026-11-09.
    media: &[MediaApi::Transcriptions],
};

const SGLANG: Server = Server {
    kind: ServerKind::Sglang,
    name: "SGLang",
    suggested_id: "sglang",
    port: 30000,
    path: "",
    owned_by: &["sglang"],
    server_header: None,
    probe: None,
    chat: local("SGLang").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("SGLang"),
    messages: MessagesProfile::compatible("SGLang"),
    // https://github.com/sgl-project/sglang/blob/4003bf1077/python/sglang/srt/managers/tokenizer_manager.py#L1437
    context_window_messages: &["exceeds the model's maximum context length"],
    media: &[],
};

const OLLAMA: Server = Server {
    kind: ServerKind::Ollama,
    name: "Ollama",
    suggested_id: "ollama",
    port: 11434,
    path: "",
    // Ollama names a model's namespace, such as `library`, which tells nothing.
    owned_by: &[],
    server_header: None,
    probe: Some(Probe {
        path: "/api/version",
        answer: Answer::OnlyString("version"),
    }),
    // Ollama drops `max_completion_tokens` and ignores `logit_bias`
    // (https://github.com/ollama/ollama/blob/d63b702609/docs/api/openai-compatibility.mdx).
    chat: local("Ollama")
        .reasoning_fields(&["reasoning"])
        .refusing(&["logit_bias"]),
    responses: ResponsesProfile::compatible("Ollama"),
    messages: MessagesProfile::compatible("Ollama"),
    context_window_messages: &[],
    media: &[],
};

const LM_STUDIO: Server = Server {
    kind: ServerKind::LmStudio,
    name: "LM Studio",
    suggested_id: "lm-studio",
    port: 1234,
    path: "",
    owned_by: &["organization_owner"],
    server_header: None,
    probe: None,
    // gpt-oss models answer with `reasoning`, others with `reasoning_content` when the
    // server's developer setting splits it out
    // (https://github.com/lmstudio-ai/docs/blob/8c7159be3f/1_developer/api-changelog.md).
    chat: local("LM Studio").reasoning_fields(&["reasoning", "reasoning_content"]),
    responses: ResponsesProfile::compatible("LM Studio"),
    messages: MessagesProfile::compatible("LM Studio"),
    context_window_messages: &[],
    media: &[],
};

const LLAMA_CPP: Server = Server {
    kind: ServerKind::LlamaCpp,
    name: "llama.cpp",
    suggested_id: "llama-cpp",
    port: 9931,
    path: "",
    owned_by: &["llamacpp"],
    server_header: Some("llama.cpp"),
    probe: None,
    chat: local("llama.cpp").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("llama.cpp"),
    messages: MessagesProfile::compatible("llama.cpp"),
    context_window_messages: LLAMA_CPP_CONTEXT,
    media: &[],
};

const LOCALAI: Server = Server {
    kind: ServerKind::Localai,
    name: "LocalAI",
    suggested_id: "localai",
    port: 8080,
    path: "",
    owned_by: &[],
    server_header: None,
    probe: Some(Probe {
        path: "/version",
        answer: Answer::OnlyString("version"),
    }),
    chat: local("LocalAI").reasoning_fields(&["reasoning", "reasoning_content"]),
    responses: ResponsesProfile::compatible("LocalAI"),
    messages: MessagesProfile::compatible("LocalAI"),
    context_window_messages: &[],
    // https://localai.io/docs/features/image-generation/, `…/text-to-audio/` and
    // `…/audio-to-text/`, read 2026-11-09.
    media: &[MediaApi::Images, MediaApi::Speech, MediaApi::Transcriptions],
};

const TGI: Server = Server {
    kind: ServerKind::Tgi,
    name: "Text Generation Inference",
    suggested_id: "tgi",
    port: 3000,
    path: "",
    // TGI names the model itself as its owner.
    owned_by: &[],
    server_header: None,
    probe: Some(Probe {
        path: "/info",
        answer: Answer::Member("router", "text-generation-router"),
    }),
    // TGI keeps `logit_bias` without using it
    // (https://github.com/huggingface/text-generation-inference/blob/b4adbf2f6e/router/src/lib.rs#L851).
    chat: local("Text Generation Inference").refusing(&["logit_bias"]),
    responses: ResponsesProfile::compatible("Text Generation Inference"),
    messages: MessagesProfile::compatible("Text Generation Inference"),
    context_window_messages: &[],
    media: &[],
};

const LMDEPLOY: Server = Server {
    kind: ServerKind::Lmdeploy,
    name: "LMDeploy",
    suggested_id: "lmdeploy",
    port: 23333,
    path: "",
    owned_by: &["lmdeploy"],
    server_header: None,
    probe: None,
    chat: local("LMDeploy").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("LMDeploy"),
    messages: MessagesProfile::compatible("LMDeploy"),
    // https://github.com/InternLM/lmdeploy/blob/a0102d4796/lmdeploy/serve/core/exceptions.py#L41
    context_window_messages: &["exceeds the model context length"],
    media: &[],
};

const NIM: Server = Server {
    kind: ServerKind::Nim,
    name: "NVIDIA NIM",
    suggested_id: "nim",
    port: 8000,
    path: "",
    owned_by: &[],
    server_header: None,
    probe: None,
    chat: local("NVIDIA NIM").reasoning_fields(&["reasoning", "reasoning_content"]),
    responses: ResponsesProfile::compatible("NVIDIA NIM"),
    messages: MessagesProfile::compatible("NVIDIA NIM"),
    context_window_messages: VLLM_CONTEXT,
    media: &[],
};

const DOCKER_MODEL_RUNNER: Server = Server {
    kind: ServerKind::DockerModelRunner,
    name: "Docker Model Runner",
    suggested_id: "docker-model-runner",
    port: 12434,
    path: "/engines",
    owned_by: &["docker"],
    server_header: None,
    probe: None,
    // Docker Model Runner passes on what its engine, llama.cpp or vLLM, answers.
    chat: local("Docker Model Runner").reasoning_fields(&["reasoning_content", "reasoning"]),
    responses: ResponsesProfile::compatible("Docker Model Runner"),
    messages: MessagesProfile::compatible("Docker Model Runner"),
    context_window_messages: &[
        "exceeds the available context size",
        "maximum context length is",
        "longer than the maximum model length",
    ],
    media: &[],
};

const KOBOLDCPP: Server = Server {
    kind: ServerKind::Koboldcpp,
    name: "KoboldCpp",
    suggested_id: "koboldcpp",
    port: 5001,
    path: "",
    owned_by: &["koboldcpp"],
    server_header: None,
    probe: None,
    chat: local("KoboldCpp").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("KoboldCpp"),
    messages: MessagesProfile::compatible("KoboldCpp"),
    context_window_messages: &[],
    media: &[],
};

const TABBYAPI: Server = Server {
    kind: ServerKind::Tabbyapi,
    name: "TabbyAPI",
    suggested_id: "tabbyapi",
    port: 5000,
    path: "",
    owned_by: &["tabbyAPI"],
    server_header: None,
    probe: None,
    chat: local("TabbyAPI").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("TabbyAPI"),
    messages: MessagesProfile::compatible("TabbyAPI"),
    context_window_messages: &[],
    media: &[],
};

const XINFERENCE: Server = Server {
    kind: ServerKind::Xinference,
    name: "Xinference",
    suggested_id: "xinference",
    port: 9997,
    path: "",
    owned_by: &["xinference"],
    server_header: None,
    probe: None,
    chat: local("Xinference").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("Xinference"),
    messages: MessagesProfile::compatible("Xinference"),
    context_window_messages: &[],
    // https://inference.readthedocs.io/en/latest/models/model_abilities/image.html and
    // `…/audio.html`, read 2026-11-09.
    media: &[MediaApi::Images, MediaApi::Speech, MediaApi::Transcriptions],
};

const MLX_LM: Server = Server {
    kind: ServerKind::MlxLm,
    name: "MLX LM",
    suggested_id: "mlx-lm",
    port: 8080,
    path: "",
    owned_by: &[],
    server_header: None,
    probe: None,
    chat: local("MLX LM").reasoning_fields(&["reasoning"]),
    responses: ResponsesProfile::compatible("MLX LM"),
    messages: MessagesProfile::compatible("MLX LM"),
    context_window_messages: &[],
    media: &[],
};

const JAN: Server = Server {
    kind: ServerKind::Jan,
    name: "Jan",
    suggested_id: "jan",
    port: 1337,
    path: "",
    // Jan names the engine that runs the model
    // (https://github.com/janhq/jan/blob/879e6ae9cc/src-tauri/src/core/server/proxy.rs#L1811).
    owned_by: &["llama.cpp", "mlx", "remote"],
    server_header: None,
    probe: None,
    chat: local("Jan").reasoning_fields(&["reasoning_content"]),
    responses: ResponsesProfile::compatible("Jan"),
    messages: MessagesProfile::compatible("Jan"),
    context_window_messages: LLAMA_CPP_CONTEXT,
    media: &[],
};

pub const SERVERS: &[Server] = &[
    VLLM,
    SGLANG,
    OLLAMA,
    LM_STUDIO,
    LLAMA_CPP,
    LOCALAI,
    TGI,
    LMDEPLOY,
    NIM,
    DOCKER_MODEL_RUNNER,
    KOBOLDCPP,
    TABBYAPI,
    XINFERENCE,
    MLX_LM,
    JAN,
];

impl ServerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ServerKind::Vllm => "vllm",
            ServerKind::Sglang => "sglang",
            ServerKind::Ollama => "ollama",
            ServerKind::LmStudio => "lm_studio",
            ServerKind::LlamaCpp => "llama_cpp",
            ServerKind::Localai => "localai",
            ServerKind::Tgi => "tgi",
            ServerKind::Lmdeploy => "lmdeploy",
            ServerKind::Nim => "nim",
            ServerKind::DockerModelRunner => "docker_model_runner",
            ServerKind::Koboldcpp => "koboldcpp",
            ServerKind::Tabbyapi => "tabbyapi",
            ServerKind::Xinference => "xinference",
            ServerKind::MlxLm => "mlx_lm",
            ServerKind::Jan => "jan",
        }
    }

    pub fn parse(s: &str) -> Option<ServerKind> {
        SERVERS.iter().map(|s| s.kind).find(|k| k.as_str() == s)
    }

    pub const fn server(self) -> &'static Server {
        match self {
            ServerKind::Vllm => &VLLM,
            ServerKind::Sglang => &SGLANG,
            ServerKind::Ollama => &OLLAMA,
            ServerKind::LmStudio => &LM_STUDIO,
            ServerKind::LlamaCpp => &LLAMA_CPP,
            ServerKind::Localai => &LOCALAI,
            ServerKind::Tgi => &TGI,
            ServerKind::Lmdeploy => &LMDEPLOY,
            ServerKind::Nim => &NIM,
            ServerKind::DockerModelRunner => &DOCKER_MODEL_RUNNER,
            ServerKind::Koboldcpp => &KOBOLDCPP,
            ServerKind::Tabbyapi => &TABBYAPI,
            ServerKind::Xinference => &XINFERENCE,
            ServerKind::MlxLm => &MLX_LM,
            ServerKind::Jan => &JAN,
        }
    }
}

// The server software whose model listing names these owners or whose answer carries this
// `server` header; `None` when neither tells.
pub fn recognize<'a>(
    owners: impl IntoIterator<Item = &'a str>,
    server_header: Option<&str>,
) -> Option<ServerKind> {
    if let Some(header) = server_header {
        let product = header.split(['/', ' ']).next().unwrap_or(header);
        if let Some(s) = SERVERS.iter().find(|s| s.server_header == Some(product)) {
            return Some(s.kind);
        }
    }
    owners.into_iter().find_map(|owner| {
        SERVERS
            .iter()
            .find(|s| s.owned_by.contains(&owner))
            .map(|s| s.kind)
    })
}

pub fn probes() -> impl Iterator<Item = (ServerKind, Probe)> {
    SERVERS.iter().filter_map(|s| s.probe.map(|p| (s.kind, p)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_server_names_itself_once_and_reads_back_from_its_name() {
        let mut names: Vec<&str> = SERVERS.iter().map(|s| s.kind.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SERVERS.len());
        for s in SERVERS {
            assert_eq!(ServerKind::parse(s.kind.as_str()), Some(s.kind));
            assert_eq!(s.kind.server().kind, s.kind);
            assert_eq!(
                serde_json::to_value(s.kind).unwrap(),
                serde_json::json!(s.kind.as_str())
            );
            assert!(
                crate::provider::ProviderId::is_valid(s.suggested_id),
                "{}",
                s.name
            );
            assert!(s.chat.provider == s.name && s.messages.provider == s.name);
        }
        assert_eq!(ServerKind::parse("vLLM"), None);
    }

    #[test]
    fn a_listing_owner_or_a_server_header_tells_the_software() {
        assert_eq!(recognize(["vllm"], None), Some(ServerKind::Vllm));
        assert_eq!(recognize(["sglang"], None), Some(ServerKind::Sglang));
        assert_eq!(
            recognize(["organization_owner"], None),
            Some(ServerKind::LmStudio)
        );
        assert_eq!(
            recognize(["docker"], None),
            Some(ServerKind::DockerModelRunner)
        );
        assert_eq!(recognize(["tabbyAPI"], None), Some(ServerKind::Tabbyapi));
        assert_eq!(recognize(["llama.cpp"], None), Some(ServerKind::Jan));
        assert_eq!(
            recognize(["library"], Some("llama.cpp")),
            Some(ServerKind::LlamaCpp)
        );
        assert_eq!(
            recognize([], Some("llama.cpp/b7187")),
            Some(ServerKind::LlamaCpp)
        );
        assert_eq!(recognize(["library", "meta-llama"], Some("uvicorn")), None);
        assert_eq!(recognize([], None), None);
    }

    #[test]
    fn a_probe_answer_matches_only_its_own_shape() {
        let ollama = Answer::OnlyString("version");
        assert!(ollama.matches(br#"{"version":"0.32.7"}"#));
        assert!(
            !ollama.matches(br#"{"error":"Unexpected endpoint or method. (GET /api/version)"}"#)
        );
        assert!(!ollama.matches(br#"{"version":"1","extra":true}"#));
        assert!(!ollama.matches(br#"{"version":1}"#));
        assert!(!ollama.matches(b"Ollama is running"));
        let tgi = Answer::Member("router", "text-generation-router");
        assert!(
            tgi.matches(br#"{"model_id":"m","router":"text-generation-router","version":"3.3.7"}"#)
        );
        assert!(!tgi.matches(br#"{"router":"other"}"#));
    }

    #[test]
    fn every_server_hears_system_messages_and_its_output_limit_as_max_tokens() {
        use kaeso_codecs::openai_chat::OutputLimit;
        for s in SERVERS {
            assert!(!s.chat.request.developer_role, "{}", s.name);
            assert_eq!(
                s.chat.request.output_limit,
                OutputLimit::MaxTokens,
                "{}",
                s.name
            );
            assert!(
                !s.chat.request.end_user && !s.chat.request.cache_key,
                "{}",
                s.name
            );
        }
        let ollama = ServerKind::Ollama.server();
        assert_eq!(ollama.chat.reply.reasoning_fields, &["reasoning"]);
        assert_eq!(
            ServerKind::LlamaCpp.server().chat.reply.reasoning_fields,
            &["reasoning_content"]
        );
        assert_eq!(ServerKind::DockerModelRunner.server().path, "/engines");
    }
}
