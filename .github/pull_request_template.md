<!-- markdownlint-disable MD041 -->
<!-- markdownlint-disable MD013 -->
## Description

<!-- Provide a concise summary of the changes and the architectural rationale. -->

## Pre-Merge Verification Checklist

- [ ] Local verification passed: `make verify-all`
- [ ] No new HISS-16 / NASA Power-of-10 infractions (all new/modified functions $\le 60$ LOC)
- [ ] 3D Tests included (Positive, Negative, Boundary) for public APIs
- [ ] Agent contexts in sync: `praetorctl compile-context --verify`
- [ ] Commit messages adhere to Conventional Commits format
