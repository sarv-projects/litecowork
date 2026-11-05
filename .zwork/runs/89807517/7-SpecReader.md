# Spec Reader — run 89807517 participant 7

**Bucket:** specs-01 — 122/122 files read, 5,631 lines (reconciles exactly with the plan), 311 KB, no truncation.

**Top 5 findings**
1. **Status conflict on 001**: root `specs/README.md` says DRAFT; 001 README/SPEC/PLAN/TASKS say ACCEPTED/LOCKED v1.0; 001 DATA-MODEL/RESEARCH/WIRING say DRAFT + `spec_revision: 0.1`. No owner sign-off evidence exists anywhere.
2. **256 KiB contradiction inside 001**: SPEC §7.5/§10 say it is the API `UserRequest` cap, "not the adapter request budget"; DATA-MODEL §3 and RESEARCH §5.4 say the OpenCode/Codex transport *is* 256 KiB/request.
3. **002–011 are one template**: PLAN, DATA-MODEL boilerplate and all four contract files are near-verbatim; real domain content is crammed into a misnumbered `## 7a` block placed after §18; every task ends with the generated "Criteria discussed include AC-001 AC-002."
4. **Traceability is vacuous in 002–011**: every row says "canonical owner docs cited in this SPEC" (no doc named), AC-001/002 are always credited to decision-only T01, UI tasks have no AC row, and 011's T02 (the IPC gate) is missing entirely.
5. **Ordering vs licensing drift**: 001 B-02 locks Windows 11/10 MSVC as first GA OS, while 002 still lists B-02 as live and 011's B-28 ("GA OS") is still open.

Digest: [reading/digest-specs-01.md](reading/digest-specs-01.md) (59 KB — dense because all 122 files are covered individually; includes a numeric-limits inventory and error-code/enum lists per package).