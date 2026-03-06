## ADDED Requirements

### Requirement: Auto-populate Description from compose project name
When a compose file has a top-level `name` field and no explicit `--description` has been provided by the user, Podlet SHALL set `Description=` in the `[Unit]` section of each generated Quadlet file using the pattern `"<entity_type> for pod <compose_name>"`, where `<entity_type>` is one of `container`, `network`, or `volume` and `<compose_name>` is the top-level compose `name` value.

#### Scenario: Container gets description when compose name is present
- **WHEN** the compose file has `name: myapp` at the top level
- **AND** no `--description` flag is passed
- **THEN** each generated `.container` file contains `Description=container for pod myapp`

#### Scenario: Network gets description when compose name is present
- **WHEN** the compose file has `name: myapp` at the top level
- **AND** no `--description` flag is passed
- **THEN** each generated `.network` file contains `Description=network for pod myapp`

#### Scenario: Volume gets description when compose name is present
- **WHEN** the compose file has `name: myapp` at the top level
- **AND** no `--description` flag is passed
- **THEN** each generated `.volume` file contains `Description=volume for pod myapp`

#### Scenario: Explicit description is not overridden
- **WHEN** the compose file has `name: myapp` at the top level
- **AND** `--description "My custom description"` is passed
- **THEN** generated files contain `Description=My custom description`
- **AND** the compose name pattern is NOT used

#### Scenario: No description when compose name is absent
- **WHEN** the compose file has no top-level `name` field
- **AND** no `--description` flag is passed
- **THEN** generated files do not contain a `Description=` line

#### Scenario: Pod mode uses pod name in description
- **WHEN** the compose file has `name: myapp` at the top level
- **AND** `--pod` is passed (making `pod_name = "myapp"`)
- **THEN** each generated file contains `Description=<entity_type> for pod myapp`
