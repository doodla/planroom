// A review round under KICKOFF's protocol: one reviewer per focus, then a separate verifier that
// tries to refute that focus's findings. Copy this into the Workflow tool's `script`, fill in the
// <...> parts, and keep `model` and `effort` literal on every agent() call (a hook checks them).
// With eight focuses, run this twice, with four focuses each: one run executes two agents at a
// time on this machine.
export const meta = {
  name: '<phase>-review-<a|b>',
  description: '<Phase> review round <N>, focuses <...>; each focus checked by a separate verifier',
  phases: [{ title: 'Review', detail: 'reviewer per focus' }, { title: 'Verify', detail: 'separate verifier per focus' }],
}

// What is under review and what is out of scope. Give the work, never the build's reasoning or
// its own view of what is wrong. Name the commit, so round N+1 can say what changed since.
const WORK = `The work under review is the Planroom repository at /home/user/planroom (branch <branch>, commit <sha>). <One paragraph: what Planroom is and where its requirements, design and invariants live: KICKOFF.md's <requirements>, <principles>, <constraints> and <acceptance>; docs/contributing/architecture.md, contracts.md, decisions.md (DR-1..DR-<n>), repo-layout.md, design-language.md, traceability.md, feature_list.json; AGENTS.md and each area's AGENTS.md.>

Scope: <what this phase built, and what later milestones will build>. Do NOT report "feature X from a later milestone isn't built" unless code or docs claim it works, or the gap makes something already built wrong or unsafe. Do report defects in what exists, drift between what exists and its docs, and design gaps the work revealed.<For round 2 and later: "Round <N-1>'s findings and their fixes are in <dir>/README.md. Check that each material fix holds, then look for what is still wrong; don't re-report a finding the README logs as a known gap unless it got worse.">

Rules for you:
- You may read anything and run read-only commands. You may run the already-built binary target/debug/planroom on a temporary copy of a fixture under a temporary directory, with PLANROOM_DATA set to a temporary directory. Do NOT run cargo, pnpm, cargo xtask, or anything that builds, writes under the repository, pushes, or calls a paid API.
- Every finding needs evidence you checked: file:line, or a command and its output. Separate what you verified from what you assume.
- Severity: "material" means it would make a tier A or B acceptance scenario fail, violates a KICKOFF principle or constraint, or is a security or data-loss risk. Everything else is "minor".
- Return at most 15 findings, the most important first. No duplicates. No style nits unless they break a documented rule.`

const FINDINGS = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'focus-n, e.g. security-3' },
          title: { type: 'string' },
          severity: { type: 'string', enum: ['material', 'minor'] },
          where: { type: 'array', items: { type: 'string' } },
          evidence: { type: 'string', description: 'what you checked and what it showed' },
          why: { type: 'string', description: 'for material: the scenario, principle, constraint or risk it hits' },
          fix: { type: 'string', description: 'the smallest fix that would close it' },
        },
        required: ['id', 'title', 'severity', 'where', 'evidence', 'why', 'fix'],
      },
    },
    summary: { type: 'string' },
  },
  required: ['findings', 'summary'],
}

const VERDICTS = {
  type: 'object',
  properties: {
    verdicts: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string' },
          refuted: { type: 'boolean' },
          severity_after: { type: 'string', enum: ['material', 'minor', 'none'] },
          reason: { type: 'string', description: 'what you checked; why it stands, is downgraded, or is refuted' },
        },
        required: ['id', 'refuted', 'severity_after', 'reason'],
      },
    },
  },
  required: ['verdicts'],
}

// One entry per focus. Pick focuses that fail in different ways; the skeleton used correctness,
// security, drift, tests (first batch) and repo, ux, operability, unasked (second batch).
const FOCUSES = [
  { key: '<focus>', ask: 'Focus: <NAME>. <What to hunt for, and which files and docs to compare.>' },
]

const results = await pipeline(
  FOCUSES,
  (f) => agent(`${WORK}\n\n${f.ask}`, { label: `review:${f.key}`, phase: 'Review', schema: FINDINGS, model: 'opus', effort: 'high' }),
  (review, f) => {
    if (!review || !review.findings || review.findings.length === 0) return { focus: f.key, review, verdicts: [] }
    return agent(`${WORK}\n\nYou are a separate VERIFIER for the "${f.key}" focus. Another reviewer reported the findings below. Try to REFUTE each one independently: re-check its evidence yourself (read the cited lines, rerun the cited command where the rules allow), look for code, tests or decision records that already handle it, and check whether its severity is right under the definition above. Default to refuted=false only when you confirmed the defect yourself. Judge each finding on its own; do not weigh one against another.\n\nFindings:\n${JSON.stringify(review.findings, null, 1)}`,
      { label: `verify:${f.key}`, phase: 'Verify', schema: VERDICTS, model: 'opus', effort: 'high' })
      .then((v) => ({ focus: f.key, review, verdicts: v ? v.verdicts : null }))
  },
)
return results
