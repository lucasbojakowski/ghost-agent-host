use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SkillSummary {
    pub name: &'static str,
    pub description: &'static str,
    pub path: &'static str,
}

struct SkillDefinition {
    summary: SkillSummary,
    content: &'static str,
}

const SKILLS: &[SkillDefinition] = &[
    SkillDefinition {
        summary: SkillSummary {
            name: "reference-production-session",
            description: "Compose reference analysis, planning, FL scaffolding, capture, arrangement and groove work into one guided production session.",
            path: "apps/ghost-fl-workspace/skills/reference-production-session/SKILL.md",
        },
        content: include_str!("../skills/reference-production-session/SKILL.md"),
    },
    SkillDefinition {
        summary: SkillSummary {
            name: "reference-analysis",
            description: "Analyze a reference mix and separated stems while keeping measurements, musical inference and creative interpretation distinct.",
            path: "apps/ghost-fl-workspace/skills/reference-analysis/SKILL.md",
        },
        content: include_str!("../skills/reference-analysis/SKILL.md"),
    },
    SkillDefinition {
        summary: SkillSummary {
            name: "fl-audio-capture",
            description: "Capture an exact live FL mixer signal through Ghost Tap with explicit slot, transport, arm/play/collect and artifact verification.",
            path: "apps/ghost-fl-workspace/skills/fl-audio-capture/SKILL.md",
        },
        content: include_str!("../skills/fl-audio-capture/SKILL.md"),
    },
    SkillDefinition {
        summary: SkillSummary {
            name: "project-scaffold",
            description: "Turn an approved Production Plan into a verified FL channel, mixer and playlist scaffold.",
            path: "apps/ghost-fl-workspace/skills/project-scaffold/SKILL.md",
        },
        content: include_str!("../skills/project-scaffold/SKILL.md"),
    },
    SkillDefinition {
        summary: SkillSummary {
            name: "arrangement-planning",
            description: "Convert temporal reference evidence plus producer language into sections, markers, roles and production intentions.",
            path: "apps/ghost-fl-workspace/skills/arrangement-planning/SKILL.md",
        },
        content: include_str!("../skills/arrangement-planning/SKILL.md"),
    },
    SkillDefinition {
        summary: SkillSummary {
            name: "groove-transcription",
            description: "Turn isolated stem rhythm/pitch projections into cautious, editable MIDI proposals and validate a short section first.",
            path: "apps/ghost-fl-workspace/skills/groove-transcription/SKILL.md",
        },
        content: include_str!("../skills/groove-transcription/SKILL.md"),
    },
];

pub(crate) fn list_skills() -> Vec<SkillSummary> {
    SKILLS.iter().map(|skill| skill.summary.clone()).collect()
}

pub(crate) fn read_skill(name: &str) -> Result<Value> {
    let name = name.trim();
    let skill = SKILLS
        .iter()
        .find(|skill| skill.summary.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow::anyhow!("unknown workspace skill `{name}`"))?;
    Ok(json!({
        "name": skill.summary.name,
        "description": skill.summary.description,
        "path": skill.summary.path,
        "content": skill.content
    }))
}

pub(crate) fn bootstrap_index() -> String {
    let mut output = String::from("AVAILABLE WORKSPACE SKILLS\n\n");
    for skill in SKILLS {
        output.push_str(skill.summary.name);
        output.push('\n');
        output.push_str("Path: ");
        output.push_str(skill.summary.path);
        output.push('\n');
        output.push_str("Description: ");
        output.push_str(skill.summary.description);
        output.push_str("\n\n");
    }
    output.push_str(
        "When a listed skill is directly relevant, call workspace_skill_read(name) before performing that workflow. Skill content is operational guidance, not evidence about current FL state.\n",
    );
    output
}

pub(crate) fn validate_skill_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("skill name must not be empty");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_skills_have_unique_names_and_frontmatter() {
        let mut names = std::collections::BTreeSet::new();
        for skill in SKILLS {
            assert!(names.insert(skill.summary.name));
            assert!(skill.content.starts_with("---\n"));
            assert!(skill
                .content
                .contains(&format!("name: {}", skill.summary.name)));
        }
    }

    #[test]
    fn capture_skill_encodes_arm_play_collect_order() {
        let capture = read_skill("fl-audio-capture").unwrap();
        let content = capture["content"].as_str().unwrap();
        let arm = content.find("ghost_tap_arm").unwrap();
        let play = content.find("Start playback").unwrap();
        let collect = content.find("ghost_tap_collect").unwrap();
        assert!(arm < play && play < collect);
    }
}
