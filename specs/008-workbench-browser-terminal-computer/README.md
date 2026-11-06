# 008 - Workbench Browser Terminal and Computer

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.3 | **Updated:** 2026-11-06
**Owners to approve:** Desktop shell, Environments, Browser provider, Trust, Desktop UI.

## User outcome

The owner uses the right Workbench to finish real browser and terminal work, takes over and returns control, and a stale handle cannot act.

## Current truth

The active Workbench Files pane is a Resource projection/preview, not a live filesystem. Browser is a sandboxed iframe preview, not Chrome or a browser-control provider. Terminal launches no process or IPC: built-in commands print samples and arbitrary commands display a fabricated successful exit code while the UI claims ConPTY/Job Object containment. Diffs are synthetic; Activity has no rendered branch; Computer capture/input is unavailable. No BrowserSession, PTY or Computer route/provider exists. The terminal's fabricated-success and containment claims are a UI truthfulness defect to remove, not a capability to qualify by documentation. This package remains DRAFT.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Specification standard](../../docs/SPECIFICATION-STANDARD.md). Domain narrative: [RESEARCH.md](RESEARCH.md).
