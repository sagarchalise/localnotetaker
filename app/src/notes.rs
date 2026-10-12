//! One markdown file per meeting. Transcript lines are appended and written through to the OS
//! immediately, so a crash never loses what was already said. On stop the file is rewritten
//! (temp file + rename) with the final frontmatter and the summary above the transcript.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use chrono::{DateTime, Local};

const TRANSCRIPT_HEADING: &str = "## Transcript";

/// One finished utterance.
#[derive(Debug, Clone)]
pub struct Line {
    pub at: DateTime<Local>,
    pub label: &'static str,
    pub text: String,
}

impl Line {
    pub fn markdown(&self) -> String {
        format!("- [{}] **{}:** {}", self.at.format("%H:%M:%S"), self.label, self.text)
    }
}

pub struct Note {
    pub path: PathBuf,
    file: File,
    started: DateTime<Local>,
}

fn frontmatter(started: DateTime<Local>, duration: &str) -> String {
    format!(
        "---\ndate: {}\nduration: {duration}\ntitle: \n---\n",
        started.format("%Y-%m-%dT%H:%M:%S")
    )
}

impl Note {
    pub fn create(dir: &Path, started: DateTime<Local>) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let stem = started.format("%Y-%m-%d_%H%M").to_string();
        let mut path = dir.join(format!("{stem}.md"));
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{stem}_{n}.md"));
            n += 1;
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("creating {}", path.display()))?;
        write!(file, "{}\n{TRANSCRIPT_HEADING}\n\n", frontmatter(started, "in progress"))?;
        Ok(Self { path, file, started })
    }

    /// Append one line; unbuffered `write_all` so it reaches the OS before returning.
    pub fn append(&mut self, line: &Line) -> anyhow::Result<()> {
        self.file.write_all(format!("{}\n", line.markdown()).as_bytes())?;
        Ok(())
    }

    /// Rewrite with final frontmatter and `summary` (markdown with its own `##` headings)
    /// placed above the transcript.
    pub fn finish(self, duration: Duration, summary: &str) -> anyhow::Result<()> {
        let Note { path, file, started } = self;
        drop(file);
        let text = std::fs::read_to_string(&path)?;
        let transcript = text
            .split_once(TRANSCRIPT_HEADING)
            .map(|(_, rest)| rest.trim_start_matches('\n'))
            .unwrap_or("");
        // Lines are appended in flush order (crash-safe); sort by start time for reading.
        let mut lines: Vec<&str> = transcript.lines().filter(|l| !l.trim().is_empty()).collect();
        lines.sort_by_key(|l| l.get(..12).unwrap_or("").to_string()); // "- [HH:MM:SS]", stable
        let transcript = format!("{}\n", lines.join("\n"));
        let secs = duration.as_secs();
        let dur = format!("{:02}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60);
        let out = format!(
            "{}{}\n\n{TRANSCRIPT_HEADING}\n\n{}",
            frontmatter(started, &dur),
            summary.trim_end(),
            transcript
        );
        let tmp = path.with_extension("md.tmp");
        std::fs::write(&tmp, out)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}
