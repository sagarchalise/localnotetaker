//! Summary with a small (<1 GB) chat model. A 0.5B-class model has a short effective context,
//! so long transcripts are reduced hierarchically: chunk -> notes, merge notes until they fit,
//! then one final pass that produces the four required sections.

use std::sync::Arc;

use foundry_local_sdk::{
    ChatSession, FoundryLocalManager, Item, Model, Request, RequestOptions, SearchOptions,
};

use crate::transcriber::{load_model, Progress};

const CHUNK_CHARS: usize = 5000;
const FINAL_INPUT_CHARS: usize = 6000;
const SECTIONS: [&str; 4] = ["## Summary", "## Decisions", "## Action items", "## Open questions"];

pub struct Summarizer {
    model: Arc<Model>,
}

impl Summarizer {
    pub async fn load(
        manager: &FoundryLocalManager,
        alias: &str,
        variant: Option<&str>,
        progress: &Progress,
    ) -> anyhow::Result<Self> {
        Ok(Self { model: load_model(manager, alias, variant, progress).await? })
    }

    pub async fn unload(&self) {
        if let Err(e) = self.model.unload().await {
            log::warn!("unloading chat model: {e}");
        }
    }

    /// Returns markdown containing exactly the four required `##` sections.
    pub async fn summarize(&self, transcript: &str) -> anyhow::Result<String> {
        let mut notes = transcript.to_string();
        let mut round = 0;
        while notes.len() > FINAL_INPUT_CHARS && round < 4 {
            round += 1;
            let chunks = split(&notes, CHUNK_CHARS);
            log::info!("summary round {round}: {} chunks", chunks.len());
            let mut next = Vec::new();
            for chunk in chunks {
                next.push(self.ask(NOTES_PROMPT, &chunk, 450).await?);
            }
            notes = next.join("\n");
        }
        let raw = self.ask(FINAL_PROMPT, &notes, 700).await?;
        Ok(normalize(&raw))
    }

    async fn ask(&self, system: &str, user: &str, max_tokens: i32) -> anyhow::Result<String> {
        let session = ChatSession::new(&self.model).await?;
        let options = RequestOptions {
            search: SearchOptions {
                temperature: Some(0.2),
                max_output_tokens: Some(max_tokens),
                ..Default::default()
            },
            ..Default::default()
        };
        let request = Request::from_items(vec![
            Item::system_message(vec![Item::text(system)]),
            Item::user_message(vec![Item::text(user)]),
        ])
        .with_options(options);
        Ok(session.process_request(request).await?.text())
    }
}

const NOTES_PROMPT: &str = "You condense part of a meeting transcript into terse bullet notes. \
Cover key points, decisions, action items (who does what) and open questions. \
Only use facts stated in the transcript. Do not invent anything.";

const FINAL_PROMPT: &str = "You write meeting notes from a transcript or from condensed notes. \
Reply in exactly this markdown format and nothing else:\n\
## Summary\n(2-4 sentences)\n## Decisions\n- bullet per decision, or - None\n\
## Action items\n- bullet per action item with owner if stated, or - None\n\
## Open questions\n- bullet per unresolved question, or - None\n\
Only use facts stated in the input. Do not invent anything.";

/// Split on line boundaries into pieces of at most `max` chars (a single longer line stays whole).
fn split(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if !cur.is_empty() && cur.len() + line.len() + 1 > max {
            out.push(std::mem::take(&mut cur));
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Small models drift from the format; guarantee the four headings exist, in order.
fn normalize(raw: &str) -> String {
    let raw = raw.trim();
    // If the model ignored the format entirely, keep its text as the summary.
    let formatted = raw.contains(SECTIONS[0]);
    let mut out = String::new();
    for (i, heading) in SECTIONS.iter().enumerate() {
        let body = if i == 0 && !formatted {
            Some(raw.to_string()).filter(|s| !s.is_empty())
        } else {
            section(raw, heading, SECTIONS.get(i + 1).copied())
        };
        let body = body.unwrap_or_else(|| "- None".into());
        // Section 0 is prose; the rest are bullet lists (small models sometimes drop the "- ").
        let body = if i > 0 && !body.starts_with(['-', '*']) { format!("- {body}") } else { body };
        out.push_str(&format!("{heading}\n{body}\n\n"));
    }
    out.trim_end().to_string()
}

fn section(raw: &str, heading: &str, next: Option<&str>) -> Option<String> {
    let start = raw.find(heading)? + heading.len();
    let rest = &raw[start..];
    let end = next.and_then(|n| rest.find(n)).unwrap_or(rest.len());
    let body = rest[..end].trim();
    (!body.is_empty()).then(|| body.to_string())
}
