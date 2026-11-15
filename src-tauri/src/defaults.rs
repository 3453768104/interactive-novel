/// Shipped defaults. They are compiled into the binary and copied to
/// `<exe dir>/data/config/` on first run, after which the copies are the
/// single source of truth (editable from the UI or a text editor).

pub const ENGINES_YAML: &str = include_str!("../../config/defaults/engines.yaml");
pub const APP_YAML: &str = include_str!("../../config/defaults/app.yaml");
pub const POLICY_API: &str = include_str!("../../config/defaults/policies/api.yaml");
pub const POLICY_LOCAL: &str = include_str!("../../config/defaults/policies/local.yaml");
pub const POLICY_PASTE: &str = include_str!("../../config/defaults/policies/paste.yaml");
pub const SYSTEM_API: &str = include_str!("../../config/defaults/templates/system.api.md");
pub const SYSTEM_LOCAL: &str = include_str!("../../config/defaults/templates/system.local.md");
pub const SYSTEM_PASTE: &str = include_str!("../../config/defaults/templates/system.paste.md");
pub const SYSTEM_OMNI_API: &str = include_str!("../../config/defaults/templates/system.omni.api.md");
pub const SYSTEM_OMNI_LOCAL: &str = include_str!("../../config/defaults/templates/system.omni.local.md");
pub const SYSTEM_OMNI_PASTE: &str = include_str!("../../config/defaults/templates/system.omni.paste.md");
pub const OUTPUT_API: &str = include_str!("../../config/defaults/templates/output.api.md");
pub const OUTPUT_LOCAL: &str = include_str!("../../config/defaults/templates/output.local.md");
pub const OUTPUT_PASTE: &str = include_str!("../../config/defaults/templates/output.paste.md");
pub const CONFIG_README: &str = include_str!("../../config/defaults/README.md");

pub const LORE_TEMPLATE: &str = "# 设定\n\n## 地理\n\n## 势力\n\n## 人物\n\n## 物品\n\n## 规则\n";

/// Bump once per release that changes any shipped default file.
/// Version 1 = original MVP defaults (frozen in config/defaults_prev/).
/// Version 2 = portable layout, event_background/timeline, one-option-per-line.
/// Version 3 = four-block output contract, layered state, POV, stop policy,
///             structured strategy panel. Baseline frozen in defaults_v2/.
/// Version 4 = lore index + pinned cards + director mode + merged character
///             tiers. Baseline frozen in defaults_v3/.
pub const DEFAULTS_VERSION: i64 = 4;

// The immediately preceding factory snapshot. Comparing against THIS (not an
// older one) is essential: a user who never hand-edited a file must be
// upgraded automatically instead of being misdetected as "modified".
const PREV_ENGINES: &str = include_str!("../../config/defaults_v3/engines.yaml");
const PREV_APP: &str = include_str!("../../config/defaults_v3/app.yaml");
const PREV_POLICY_API: &str = include_str!("../../config/defaults_v3/policies/api.yaml");
const PREV_POLICY_LOCAL: &str = include_str!("../../config/defaults_v3/policies/local.yaml");
const PREV_POLICY_PASTE: &str = include_str!("../../config/defaults_v3/policies/paste.yaml");
const PREV_SYSTEM_API: &str = include_str!("../../config/defaults_v3/templates/system.api.md");
const PREV_SYSTEM_LOCAL: &str = include_str!("../../config/defaults_v3/templates/system.local.md");
const PREV_SYSTEM_PASTE: &str = include_str!("../../config/defaults_v3/templates/system.paste.md");
const PREV_SYSTEM_OMNI_API: &str =
    include_str!("../../config/defaults_v3/templates/system.omni.api.md");
const PREV_SYSTEM_OMNI_LOCAL: &str =
    include_str!("../../config/defaults_v3/templates/system.omni.local.md");
const PREV_SYSTEM_OMNI_PASTE: &str =
    include_str!("../../config/defaults_v3/templates/system.omni.paste.md");
const PREV_OUTPUT_API: &str = include_str!("../../config/defaults_v3/templates/output.api.md");
const PREV_OUTPUT_LOCAL: &str = include_str!("../../config/defaults_v3/templates/output.local.md");
const PREV_OUTPUT_PASTE: &str = include_str!("../../config/defaults_v3/templates/output.paste.md");
const PREV_README: &str = include_str!("../../config/defaults_v3/README.md");

/// The exact v2 factory files, used for three-way sha comparison on upgrade.
pub fn prev_all() -> Vec<(&'static str, &'static str)> {
    vec![
        ("engines.yaml", PREV_ENGINES),
        ("app.yaml", PREV_APP),
        ("policies/api.yaml", PREV_POLICY_API),
        ("policies/local.yaml", PREV_POLICY_LOCAL),
        ("policies/paste.yaml", PREV_POLICY_PASTE),
        ("templates/system.api.md", PREV_SYSTEM_API),
        ("templates/system.local.md", PREV_SYSTEM_LOCAL),
        ("templates/system.paste.md", PREV_SYSTEM_PASTE),
        ("templates/system.omni.api.md", PREV_SYSTEM_OMNI_API),
        ("templates/system.omni.local.md", PREV_SYSTEM_OMNI_LOCAL),
        ("templates/system.omni.paste.md", PREV_SYSTEM_OMNI_PASTE),
        ("templates/output.api.md", PREV_OUTPUT_API),
        ("templates/output.local.md", PREV_OUTPUT_LOCAL),
        ("templates/output.paste.md", PREV_OUTPUT_PASTE),
        ("README.md", PREV_README),
    ]
}

/// (relative path under config/, file content)
pub fn all() -> Vec<(&'static str, &'static str)> {
    vec![
        ("engines.yaml", ENGINES_YAML),
        ("app.yaml", APP_YAML),
        ("policies/api.yaml", POLICY_API),
        ("policies/local.yaml", POLICY_LOCAL),
        ("policies/paste.yaml", POLICY_PASTE),
        ("templates/system.api.md", SYSTEM_API),
        ("templates/system.local.md", SYSTEM_LOCAL),
        ("templates/system.paste.md", SYSTEM_PASTE),
        ("templates/system.omni.api.md", SYSTEM_OMNI_API),
        ("templates/system.omni.local.md", SYSTEM_OMNI_LOCAL),
        ("templates/system.omni.paste.md", SYSTEM_OMNI_PASTE),
        ("templates/output.api.md", OUTPUT_API),
        ("templates/output.local.md", OUTPUT_LOCAL),
        ("templates/output.paste.md", OUTPUT_PASTE),
        ("README.md", CONFIG_README),
    ]
}
