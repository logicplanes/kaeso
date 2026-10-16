use super::{OrgPreset, ProjectPreset};

impl OrgPreset {
    pub const fn description(self) -> &'static str {
        match self {
            OrgPreset::Owner => {
                "Holds every right, now and in future releases; only owners manage owners."
            }
            OrgPreset::Admin => {
                "Manages the organization and every project, except deleting it, transferring ownership and changing billing."
            }
            OrgPreset::Billing => "Manages billing and sees usage and costs across all projects.",
            OrgPreset::Developer => {
                "Creates projects and sees usage, costs and provider keys; works in the projects they are added to."
            }
            OrgPreset::Auditor => {
                "Reads the whole organization for reviews, including the audit log and every project, and changes nothing; never sees prompts or outputs."
            }
            OrgPreset::Member => {
                "Sees the organization and its members; works in the projects they are added to."
            }
        }
    }
}

impl ProjectPreset {
    pub const fn description(self) -> &'static str {
        match self {
            ProjectPreset::Admin => {
                "Manages the project: members, keys, service accounts, models, limits, logs and settings."
            }
            ProjectPreset::Developer => {
                "Calls models with their own keys and in the Playground, and reads the project, including prompts and outputs."
            }
            ProjectPreset::Viewer => {
                "Reads the project's settings, models, limits, keys, usage and log metadata; changes nothing."
            }
        }
    }
}
