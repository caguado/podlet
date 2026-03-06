## ADDED Requirements

### Requirement: Compose network `name` maps to `NetworkName=`
When a compose network definition includes a `name` field, Podlet SHALL emit `NetworkName=<value>` in the generated `.network` Quadlet file. The generated file's own name (and thus the systemd unit name) SHALL remain based on the compose map key, not the custom name. Conversion SHALL succeed without error.

#### Scenario: Network with custom name generates NetworkName field
- **WHEN** a compose file contains a network with a `name` field (e.g., `name: my-custom-net`)
- **THEN** the generated `.network` file contains `NetworkName=my-custom-net`

#### Scenario: Network without custom name omits NetworkName field
- **WHEN** a compose file contains a network with no `name` field
- **THEN** the generated `.network` file does not contain a `NetworkName=` line

#### Scenario: File name is based on compose map key, not custom name
- **WHEN** a compose network with map key `backend` has `name: actual-backend`
- **THEN** the generated file is named `backend.network` (not `actual-backend.network`)
- **AND** the file contains `NetworkName=actual-backend`
