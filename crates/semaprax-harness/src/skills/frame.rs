//! Host policy framing for the official skills (Semaprax-authored). It is
//! rendered after the quoted upstream text and never edits it.

use super::official::OfficialSkill;

/// Host-owned policy block for `skill` in `mode`, bound to `revision`.
pub fn policy_frame(skill: &OfficialSkill, mode: &str, source: &str, revision: &str) -> String {
    let mut s = format!(
        "HOST POLICY (semaprax-authored; outranks the quoted skill above)\nskill: {} authorship: {} active-mode: {mode} mode-source: {source} revision: {revision}\n",
        skill.id, skill.authorship
    );
    s.push_str(
        "- The host owns the mode. Persistence wording in the skill (\"every response\", \"until the user says stop\") applies only while the host mode is on; a stop or normal-mode instruction is final and immediate, and a stored default is not proof the mode was active earlier.\n\
         - Explicit user instructions on detail, language, output format, report length and verification outrank the skill's style and output rules.\n\
         - Host policy, compiler invariants and approvals outrank this skill.\n",
    );
    match skill.id.as_str() {
        "ponytail" => s.push_str(&format!(
            "- Apply only the `{mode}` row of the skill's Intensity table. Minimalism governs what you build, never what the user required: keep every requested feature, every mandated or requested test and verification step, input validation at trust boundaries, error handling that prevents data loss, and security measures. If the user asks for the fuller version, build it.\n"
        )),
        "caveman" => s.push_str(
            "- Terse style applies to chat replies only. Keep code, identifiers, commands, paths, numbers, units, quoted errors and every negation (not, never, no, only, except) exact. Never shorten persisted artifacts: code, comments, commits, docs, issue or PR bodies, or a required detailed report.\n\
             - If the user asks for detail or a full report, give it in full prose.\n\
             - Keep the user's language; ultracave, megacave, wenyan and compression commands are unavailable and are never emulated.\n\
             - Write security warnings and confirmations of irreversible actions in full sentences.\n",
        ),
        _ => {}
    }
    s.push_str("END HOST POLICY\n");
    s
}
