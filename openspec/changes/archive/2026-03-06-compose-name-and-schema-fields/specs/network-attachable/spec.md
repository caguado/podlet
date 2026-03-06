## ADDED Requirements

### Requirement: Compose network `attachable` maps to PodmanArgs `--attachable`
When a compose network definition has `attachable: true`, Podlet SHALL append `--attachable` to the `PodmanArgs=` value in the generated `.network` Quadlet file. When `attachable` is `false` (the default), no `--attachable` flag SHALL be emitted. Conversion SHALL succeed without error (the field SHALL no longer be rejected).

#### Scenario: Attachable network emits flag in PodmanArgs
- **WHEN** a compose network has `attachable: true`
- **THEN** the generated `.network` file contains `PodmanArgs=--attachable` (or includes `--attachable` within a larger `PodmanArgs=` value)

#### Scenario: Non-attachable network emits no flag
- **WHEN** a compose network has `attachable: false` or no `attachable` field
- **THEN** the generated `.network` file does not contain `--attachable`

#### Scenario: Attachable combined with other PodmanArgs
- **WHEN** a compose network has `attachable: true` and other options that also produce `PodmanArgs=`
- **THEN** the `--attachable` flag appears in the same `PodmanArgs=` line alongside the other flags
