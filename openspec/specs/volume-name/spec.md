## ADDED Requirements

### Requirement: Compose volume `name` maps to `VolumeName=`
When a compose volume definition includes a `name` field, Podlet SHALL emit `VolumeName=<value>` in the generated `.volume` Quadlet file. The generated file's own name SHALL remain based on the compose map key. Containers referencing the volume via the compose key SHALL continue to use the `.volume` file reference (e.g., `mykey.volume:/path`); Quadlet resolves the runtime volume name via `VolumeName=`. Conversion SHALL succeed without error.

#### Scenario: Volume with custom name generates VolumeName field
- **WHEN** a compose file contains a volume with a `name` field (e.g., `name: persistent-data`)
- **THEN** the generated `.volume` file contains `VolumeName=persistent-data`

#### Scenario: Volume without custom name omits VolumeName field
- **WHEN** a compose file contains a volume with no `name` field
- **THEN** the generated `.volume` file does not contain a `VolumeName=` line

#### Scenario: File name is based on compose map key, not custom name
- **WHEN** a compose volume with map key `data` has `name: app-data`
- **THEN** the generated file is named `data.volume` (not `app-data.volume`)
- **AND** the file contains `VolumeName=app-data`

#### Scenario: Container volume reference is unchanged
- **WHEN** a service mounts volume with map key `data` that has `name: app-data`
- **THEN** the container file contains `Volume=data.volume:/container/path`
- **AND** the `.volume` file contains `VolumeName=app-data`
