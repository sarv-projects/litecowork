# Rich Response Design

This is optional, zero-authority guidance for preparing human-facing Conversation
responses. LiteCowork still validates every reference and renders system-owned status from
trusted projections. This Skill cannot create Tasks, Artifacts, Approvals, Verification,
Effects, downloads, grants, or capabilities.

## 1. Mission

Give the user a clear, complete answer in the simplest useful form. A RichPresentation may
improve comprehension, but it never replaces the semantic answer.

## 2. Non-negotiable rules

- Write the complete semantic answer first.
- Reference only exact Resource, ArtifactVersion, and Invocation refs supplied for this
  turn. Never invent IDs, URLs, paths, statuses, sources, or verification.
- Treat `litecowork.presentation.propose` as a layout hint, not an action or state command.
- Use host-provided system blocks only when asked and supported; the host supplies their
  actual current data.
- Every chart/diagram has an accessible textual summary. Cite exact pinned sources for
  factual material when available.
- If uncertain whether rich structure helps, use prose.

## 3. Decide plain versus rich

Prefer prose for a short fact, casual response, small troubleshooting answer, or concise
code answer. Rich structure helps with comparisons, substantial research, quantitative
results, architecture/process explanations, media groups, and several existing outputs.
Never add cards/charts merely because they are available.

## 4. Choose width and density

Keep prose readable-width. Use wide layout for tables, diagrams, and charts. Prefer
compact density for one-screen answers and detailed density only when the user needs the
supporting material.

## 5. Component selection

- Short answer: prose.
- Ordered instructions: numbered prose or a small checklist.
- Two to five alternatives: cards or a comparison table.
- Many comparable fields: table.
- Measured numeric relationship: chart backed by a bound result/resource.
- Architecture/process: diagram with an accessible summary.
- Dated sequence: timeline.
- Existing generated files: DeliverableGroup with exact ArtifactVersion refs.
- Many files: paginated ArtifactCollection.
- Evidence: citations bound to exact source revisions.
- Actual Task/Approval/UserRequest state: request the host projection; do not draw it as
  model content.

## 6. Tables and charts

Do not invent or interpolate measurements. Keep tables concise and identify units. A chart
must come from a supplied structured result or a semantic table and include a short plain
language explanation and accessible fallback.

## 7. Diagrams and timelines

Use a diagram only when relationships or sequence are clearer visually. Keep nodes and
labels short. Summarize it in text. Do not animate live-work status; any highlighting is
user-controlled playback.

## 8. Images and media

Use only supplied authorized Resource refs, with accurate alt text. Never submit a direct
media URL for the client to fetch. Do not autoplay or loop video.

## 9. Sources and citations

Attach a citation only when the source ref and exact revision/locator are supplied. A
citation identifies supporting material; it does not by itself prove a claim.

## 10. Deliverables

List only committed ArtifactVersions. Name the exact file and useful type. Do not claim a
download or create a ZIP; Task/Artifact services and an explicit owner request are required
for file creation or bundling.

## 11. Checklists and forms

An informal checklist is local presentation state. It is not a Task Step, UserRequest,
Approval, or Verification. Actual questions/approvals use their trusted host controls.

## 12. Accessibility and motion

Keep reading order logical. Use text labels in addition to color. Provide text equivalents
for charts/diagrams and useful alt text for media. Do not rely on animation, hover, or
color to communicate meaning.

## 13. Responsive behavior

Do not assume a wide monitor. Group related content so rows can stack without changing
meaning or source order.

## 14. Anti-patterns

- A card for every paragraph.
- A chart with no measured data.
- A checkmark that implies verified completion.
- A fake approval/download/runtime control.
- A huge wall of files instead of a collection.
- Decorative motion that implies activity.
- Rich structure that hides the actual answer.

## 15. Final self-check

Can the user understand the response if all rich blocks disappear? Are sources and file
versions real? Is the layout simpler than prose for this answer? Are system status and
actions host-owned? If any answer is no, revise toward semantic prose and verified refs.
