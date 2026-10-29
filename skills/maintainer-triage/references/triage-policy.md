# Maintainer Triage Policy

Use this reference when classifying repository activity or drafting a response.

## Classification

Choose the narrowest applicable kind:

- **Bug:** observed behavior conflicts with a documented or reasonably expected
  contract.
- **Compatibility:** behavior differs from a target ABI, API, file format, or
  supported environment.
- **Security:** a plausible confidentiality, integrity, availability, isolation,
  privilege, or supply-chain impact exists.
- **Performance:** reproducible resource or latency regression with a meaningful
  workload.
- **Feature:** new behavior rather than correction of an existing contract.
- **Documentation:** implementation is correct but guidance is missing or wrong.
- **Question/support:** no defect or change is yet established.

Recommended priorities should follow repository policy. If none exists, use
plain-language impact rather than inventing a severity scale.

## Evidence standards

Prefer, in order:

1. a minimal deterministic reproduction;
2. an existing failing test or operational gate;
3. code-path analysis tied to a concrete input;
4. logs with version and environment information;
5. reporter assertion requiring confirmation.

Record the environment, revision, commands, and actual result. A report may be
credible without being reproduced, but its status must remain explicit.

## Duplicate handling

Call an item a duplicate only when the underlying behavior and required fix are
the same. Similar symptoms, shared subsystems, or matching error strings are
not sufficient. Identify the canonical item and explain the connection.

## Security reports

- Do not request public proof-of-concept exploitation or sensitive details.
- Route plausible nonpublic vulnerabilities through the repository's private
  security process.
- Minimize reproduction scope and avoid testing systems not explicitly in
  scope.
- Do not publish exploitability conclusions beyond the available evidence.
- Preserve embargoes and confidential reporter information.

## Response patterns

### Confirmed

Thank the contributor, state what was reproduced, identify the affected
contract, and name the next gate or action. Avoid promising a completion date
without authority.

### Needs information

State what is known, list only the missing facts needed to continue, and provide
a minimal command or template for collecting them.

### Not reproduced

List the revision and environment tested, describe the observed result, and ask
for the smallest differentiating information. Do not imply the report is false.

### Pull request changes requested

Separate blocking correctness or compatibility findings from optional design
suggestions. Tie each blocking request to code, a test, an architecture rule, or
an observable failure.

### Declined

Explain the architectural, scope, compatibility, or maintenance reason. Keep
the response about the contribution rather than the contributor.

## Monitoring summary

For a batch, use a compact table or grouped list containing:

- item;
- classification;
- current status;
- material change since the last review;
- recommended action; and
- authorization needed.

Do not flood the report with unchanged, automated, or low-signal activity.
