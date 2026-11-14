use std::fmt;
use std::path::{Component, Path};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

pub const EVENT_PREFIX: &str = "ritual.feedback.";
pub const SCHEMA: u32 = 1;
const MAX_CHECK_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Think,
    Spec,
    Plan,
    Build,
    Review,
    Ship,
    Reflect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Kind {
    /// A step finished and handed over its file.
    StepDone,
    /// A gate refused to let the flow move on.
    GateFailed,
    /// Work was redone because the step's output was wrong.
    Rework,
    /// The user corrected the agent.
    Correction,
    /// Code and design diverged.
    Drift,
    /// The skill's instructions were unclear, missing or wrong.
    Friction,
    /// Something in the skill worked and should be kept.
    Praise,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::StepDone => "step_done",
            Kind::GateFailed => "gate_failed",
            Kind::Rework => "rework",
            Kind::Correction => "correction",
            Kind::Drift => "drift",
            Kind::Friction => "friction",
            Kind::Praise => "praise",
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Step::Think => "think",
            Step::Spec => "spec",
            Step::Plan => "plan",
            Step::Build => "build",
            Step::Review => "review",
            Step::Ship => "ship",
            Step::Reflect => "reflect",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum Skill {
    #[serde(rename = "ritual")]
    #[value(name = "ritual")]
    Ritual,
    #[serde(rename = "ritual-full")]
    #[value(name = "ritual-full")]
    RitualFull,
}

impl Skill {
    pub fn dir(self) -> &'static str {
        match self {
            Skill::Ritual => "ritual",
            Skill::RitualFull => "ritual-full",
        }
    }
}

/// What the skill text must, or must not, say for this feedback to count as addressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Check {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_not: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feedback {
    pub schema: u32,
    pub repo: String,
    pub epic: String,
    pub step: Step,
    pub kind: Kind,
    pub skill: Skill,
    pub agent: String,
    pub note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Check>,
}

impl Feedback {
    pub fn event_type(&self) -> String {
        format!("{EVENT_PREFIX}{}", self.kind.as_str())
    }

    pub fn entity_id(&self) -> String {
        format!("{}/{}", self.repo, self.epic)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("repo", &self.repo),
            ("epic", &self.epic),
            ("note", &self.note),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must not be empty"));
            }
        }
        if let Some(reference) = &self.reference {
            validate_reference(reference)?;
            if reference.starts_with("scripts/") {
                return Err(
                    "--reference must be skill prose; scripts/ is the tool's own source".into(),
                );
            }
        }
        if let Some(check) = &self.check {
            if self.reference.is_none() {
                return Err("a check needs --reference: the skill file it applies to".into());
            }
            if check.must.is_none() && check.must_not.is_none() {
                return Err("a check needs --must or --must-not".into());
            }
            for phrase in [&check.must, &check.must_not].into_iter().flatten() {
                let len = phrase.trim().chars().count();
                if len == 0 || len > MAX_CHECK_CHARS {
                    return Err(format!(
                        "check phrases must be 1..={MAX_CHECK_CHARS} characters"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A reference is a path inside the skill directory, such as `SKILL.md` or `references/build.md`.
pub fn validate_reference(reference: &str) -> Result<(), String> {
    let path = Path::new(reference);
    let inside = !reference.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && path.extension().is_some_and(|e| e == "md");
    if inside {
        Ok(())
    } else {
        Err(format!(
            "--reference must be a .md path inside the skill directory, got {reference:?}"
        ))
    }
}
