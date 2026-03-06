## ADDED Requirements

### Requirement: Compose service `domain_name` maps to PodmanArgs `--domainname`
When a compose service has a `domain_name` field set, Podlet SHALL pass `--domainname=<value>` in the `PodmanArgs=` of the generated `.container` Quadlet file. Setting `domain_name` SHALL no longer cause conversion to fail with an "unsupported" error.

#### Scenario: Service with domain_name emits PodmanArgs flag
- **WHEN** a compose service has `domain_name: example.local`
- **THEN** the generated `.container` file contains `--domainname=example.local` within `PodmanArgs=`

#### Scenario: Service without domain_name emits no flag
- **WHEN** a compose service has no `domain_name` field
- **THEN** the generated `.container` file does not contain `--domainname` in `PodmanArgs=`

#### Scenario: domain_name no longer causes an error
- **WHEN** a compose service has `domain_name` set
- **THEN** conversion succeeds without returning an error
