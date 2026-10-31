# Specification Workflow

This directory contains the feature-SDD process, templates and documentation-only validator.

Read [the Specification Standard](../docs/SPECIFICATION-STANDARD.md). Create packages under `specs/`; do not put product code or runtime services here.

1. Read the [feature package index](../specs/README.md), create a stable feature folder and copy every required template.
2. Read architecture, product, owner-domain, accepted ADR and machine contracts.
3. Complete SPEC, DATA-MODEL, RESEARCH and WIRING before sequencing implementation.
4. Trace each requirement through canonical contracts, tasks and executable acceptance.
5. Submit the full package for review. Keep DRAFT/IN_REVIEW until authorized owners accept and lock it.
6. Run `python3 .specify/scripts/validate_feature_package.py`.
7. Only ACCEPTED/LOCKED packages can enter implementation.

Reuse canonical global definitions by link. Mark exact signatures CURRENT or PROPOSED. Give every unknown an owner and gate. Keep `CURRENT-RUN.md` current. Validation is not owner approval or runtime qualification.
