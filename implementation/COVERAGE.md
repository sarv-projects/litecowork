# Coverage map

Baseline 731 concrete concerns/cases are listed in `coverage.csv`: 46 authority docs, 174 OpenAPI operations, 18 event schema definitions and 133 registered typed event values, 105 SQL tables, 140 SQL triggers, 76 SQL indexes, 19 delegation schema definitions, plus all existing numbered Flows/Benchmarks. Entries map to a primary guardian/story for planning; this does not mean the story alone implements unrelated domain changes. Domain story owners update the contracts and tests.

Read by current authority in the [audit inventory](audit-inventory.csv). Baseline line count and SHA-256 make later drift visible. Large contracts received structural enumeration and architecture CI validation; a development agent must inspect exact affected definitions and linked transitions. After changing any flow, benchmark, operation, event, enum, table, trigger or index, update this map and rerun the plan checker. A new contract concern without a story fails validation.
