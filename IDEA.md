## Project description

CasVPS is a self-hosted virtualization management platform for homelabs and larger server environments. It gives administrators a web interface and API to manage virtual machines, containers, storage, networking, and the hosts that run them. The project aims to make those capabilities manageable from a small ARM-based server through to a multi-node datacenter, with monitoring, backups, security controls, and recovery features included in the same product.

## Project variables

project_name: casvps
project_org: webappsrust
# STABLE — set at first-time setup; change only on an explicit rename or fork reset
internal_name: casvps
# STABLE — original repository identity in project history; preserved across the move to webappsrust
internal_org: casapps
app_name: CasVPS
resource_label: workload
resource_label_plural: workloads

## Business logic

### Product scope & non-goals

CasVPS manages virtualization hosts and the resources running on them. Its intended users are homelab administrators and operators of larger server environments who need one place to provision and operate virtual machines, containers, and their supporting infrastructure.

The product scope includes:

- Create, inspect, configure, start, stop, restart, and remove virtual machines and containers.
- Manage VM snapshots, migration, images and installation media, and guest lifecycle settings.
- Organize storage pools and volumes; schedule, retain, and restore backups.
- Create and manage virtual networks, address allocations, and network services used by virtualized workloads, including the network services needed for PXE-based guest installation.
- View host, node, VM, container, storage, and network health and metrics; configure alerts and service checks.
- Operate multiple hosts as a cluster, including node membership, workload placement, high availability, live migration, and rolling maintenance.
- Manage operating-system media and guest templates, including Windows and macOS guests where licensing and platform requirements permit.
- Manage certificates, firewall policy, geographic risk data, intrusion and malware findings, security events, and system compliance profiles relevant to virtualization operations.
- Schedule operational tasks and workload boot order; use resource monitoring and deterministic recovery rules to detect and respond to common host or workload failures.
- Produce operational reports and notifications for workload, host, cluster, security, and backup events.
- Integrate with external identity providers and security event systems when an operator enables those integrations.
- Provide a browser-based management experience and an API suitable for automation. Preserve the documented Proxmox VE API compatibility surface for clients that rely on it.
- Provide operational support information, event and audit history, and recovery actions that help administrators diagnose and recover from failures.

CasVPS is a virtualization management product. It does not promise that every optional enterprise integration or compliance profile is enabled in every deployment. It does not provide public cloud hosting, a general-purpose application marketplace, or a managed service operated on behalf of customers.

#### Browser pages

| Path | Title | Purpose | Data shown or changed |
|------|-------|---------|-----------------------|
| /vms | Virtual machines | View and operate virtual machines available to the caller. | VM inventory, configuration, lifecycle state, snapshots, and migration status. |
| /containers | Containers | View and operate containers available to the caller. | Container inventory, configuration, and lifecycle state. |
| /server/{admin_path}/networks | Networks | Let administrators inspect and manage virtual networks. | Network configuration, address ranges, and attached workloads. |
| /storage | Storage | Let users manage authorized volumes and let administrators manage pools and backups. | Authorized volumes, pools, backups, restore points, and retention status. |
| /server/{admin_path}/monitoring | Monitoring | Let administrators review health and monitoring information and manage alerts. | Host and node metrics, workload status, service checks, and alerts. |
| /server/{admin_path}/system | System | Review host status and manage installation settings. | System health, capacity, network state, and system settings. |
| /server/{admin_path}/users | Users | Let administrators manage resource-user accounts and permissions. | Account identity, role, enabled state, and assigned access. |
| /server/{admin_path}/nodes | Cluster nodes | Review cluster membership and manage node membership. | Node identity, address, role, health, and join time. |
| /server/{admin_path}/tokens | API tokens | Let administrators review, create, and revoke managed API tokens. | Token owner, scopes, creation time, and expiry; a new token is shown only at creation. |
| /server/{admin_path}/certificates | Certificates | Review and manage certificates used by the installation. | Domain, certificate type, expiry, and renewal state. |
| /server/{admin_path}/backup-jobs | Backup jobs | Create, review, update, and remove scheduled backup jobs. | Job name, schedule, source, destination, and enabled state. |
| /server/{admin_path}/compliance | Compliance | Review and configure supported compliance profiles. | Profile state, settings, and assessment results. |
| /server/{admin_path}/audit-logs | Audit history | Search administrative and workload audit events. | Actor, action, target, outcome, and time. |
| /server/{admin_path}/security/events | Security events | Review detected security events. | Event type, affected host or workload, severity, and time. |

### Roles & permissions

- **Server administrator:** configures and operates the CasVPS installation, hosts, cluster, security settings, and all managed resources.
- **Resource user:** uses resources assigned to them and may perform permitted lifecycle and configuration actions on those resources. They must not gain access to another user's resources or host-wide administrative controls.
- **Automation client:** acts within the permissions granted to its credential and is intended for API-based administration or resource operations.

Administrative actions and resource access must be attributable to the responsible human or automation identity. The product must keep host-wide control distinct from permissions over an individual user's workloads.

### Data model & sensitivity

The product's core entities and relationships are:

- **Hosts and cluster nodes:** each node has an identity, availability, capacity, health, and cluster membership. A workload may be placed on a node; a cluster groups nodes and reports their combined resources and status.
- **Virtual machines and containers:** each workload has a name, type, guest configuration, lifecycle state, assigned resources, owner, and placement. A workload may be associated with storage, networks, images, snapshots, backups, and operational events.
- **Storage and backups:** pools contain or provide volumes; workloads may use volumes; snapshots belong to workloads; backup jobs select workloads or storage sources and produce restore points with retention information.
- **Networks and addresses:** virtual networks have network configuration and address ranges; addresses are allocated to workloads or other managed resources.
- **Operating system media and templates:** each catalog item has a source, version, architecture, availability, and local inventory state; workloads may be created from selected media or templates.
- **Monitoring and operations:** metrics and service checks describe hosts, nodes, workloads, storage, and networks; alerts refer to monitored conditions; scheduled work, events, and audit records describe operations and their outcomes.
- **Security and compliance state:** certificates, security findings, policy settings, and compliance results belong to the installation or a managed node.
- **Identity and access information:** administrator and resource-user identities have roles and permissions; API credentials belong to an identity and may carry scopes and expiry metadata.

Host configuration, network topology, guest inventories, credentials, certificates, and security findings are sensitive operational data. VM and container contents may contain customer or personal data and must be treated as untrusted workload data. Logs and monitoring data can reveal infrastructure details and user activity.

### User flows

1. An administrator opens the management interface, completes initial setup, and reviews host capacity and health.
2. The administrator prepares storage, networking, and installation media, then creates a VM or container and assigns its resources and owner.
3. An authorized user or administrator inspects the workload, performs permitted lifecycle actions, and reviews its status and events.
4. An administrator creates snapshots or backup schedules and restores a workload when recovery is needed.
5. For a multi-node installation, an administrator joins hosts to a cluster, reviews placement and node health, and migrates or recovers workloads as required.
6. An automation client uses the API to perform the same authorized resource and operations tasks without relying on browser interaction.

### Business rules & constraints

- A workload's owner and permissions determine who may inspect or change it; resource users cannot administer the host or access another user's workloads.
- VM and container operations must account for available host capacity and the workload's current lifecycle state.
- Destructive operations such as deleting workloads, storage, snapshots, or backups must not silently affect unrelated resources.
- Backup and restore workflows must preserve the workload data needed for recovery and make the selected restore point clear to the administrator.
- Cluster operations must account for node availability and workload placement; a failed node must not be represented as healthy.
- Proxmox VE compatibility is a client interoperability goal for the documented API surface; CasVPS-specific capabilities may extend beyond that surface.
- Security scanners, compliance profiles, and enterprise integrations are product capabilities whose availability and activation may depend on the installation and its configuration.

### Trust boundaries & external services

- Administrators, resource users, automation clients, unauthenticated network callers, managed guests, and cluster peers are separate trust domains. A caller must not gain more access by changing an object identifier, node name, or request payload.
- VM and container guests are untrusted, including guests created from operator-supplied or externally sourced media. Guest activity must not grant access to the host, management plane, another workload, or cluster credentials.
- Network requests, uploaded media, templates, imported images, and data returned by external services are untrusted inputs. They may be malformed, oversized, malicious, stale, or crafted to exhaust host resources.
- Cluster peers are trusted only as configured members. A compromised or unreachable peer must not silently authorize administrative changes, report false health, or corrupt unrelated workload state.
- Public ISO and operating-system image sources, update release services, security-data publishers, and remote backup destinations are external dependencies. Their content is not trusted solely because it came from a listed or popular source. An unavailable or invalid source must not be treated as proof that an update, image, backup, or security feed is valid.
- LDAP, OIDC, and SAML identity providers may be used for enterprise authentication when enabled. They are authoritative only for the identity assertions they provide; their availability or compromise must not grant host-wide rights beyond the CasVPS role and permissions assigned to that identity.
- Proxmox-compatible clients are external callers and receive only the documented compatibility operations under the permissions granted to their credentials.

### Threat model & abuse cases

**Primary assets:** host and cluster control, guest workloads and their contents, storage volumes and backups, network topology and address allocations, administrator and user identities, API credentials, certificates and private keys, security findings, and audit history.

**Attacker goals:** gain host or cluster control, access or alter another party's workload, steal credentials or guest data, disrupt workload availability, exhaust host capacity, persist through a compromised guest, or conceal destructive activity.

**Relevant threats and required product outcomes:**

- Credential theft, credential stuffing, and token misuse must not let a caller exceed the permissions of the authenticated identity.
- Object-ID substitution and malformed requests must not cross workload, storage, or tenant boundaries or trigger actions against unrelated resources.
- Malicious guests must not reach host control or other guests; guest compromise is treated as a possible event, not as proof that the host remains trustworthy.
- Malicious or oversized uploads, imported images, and API requests must not be allowed to execute as trusted host content or exhaust resources without bounds.
- Forged, replayed, or malicious cluster messages must not authorize membership or workload changes. Loss of a peer must be visible as degraded or unavailable state rather than healthy state.
- Tampered, incomplete, or unavailable backup sources must not be represented as verified recovery points. Recovery must make the chosen source and affected workload clear.
- Unauthorized changes or tampering attempts must not be silently erased from the operational record.
- External identity, image, update, security-data, and backup services may be unavailable or compromised. Their failures must be visible and must not silently weaken access controls or validate untrusted content.

### Security decisions & exceptions

- CasVPS intentionally manages host-level virtualization, networking, storage, and security functions. This powerful control is the product's purpose and is reserved for server administrators; resource users receive only workload-scoped access.
- The project intentionally includes a Proxmox VE compatibility API for interoperability. Compatibility is limited to the documented surface; it does not promise complete behavioral parity with Proxmox VE.
- The product may retrieve operating-system media, update metadata, security data, and backups from external sources because these are part of its management and recovery workflows. Those sources remain untrusted inputs, and their failure must be surfaced rather than silently accepted.
- Enterprise identity providers are an optional integration. An external identity assertion does not itself grant CasVPS administrative privileges; CasVPS permissions remain authoritative.
- No security exception authorizes a guest, external service, or compatibility client to bypass CasVPS authorization or access another workload.

### API routes

These are the project-defined API operations for CasVPS. The template's own authentication, health, documentation, and generic server-administration routes are not repeated here. Resource routes require an authenticated caller with permission for the target resource; administrative routes require a server administrator. Request and response summaries state the product contract at the level supported by project history.

| Method | Path | Auth | Purpose | Request / response summary |
|--------|------|------|---------|----------------------------|
| GET | /api/{api_version}/server/{admin_path}/system/status | Server administrator | Read host and workload summary. | No body; returns version, uptime, node/workload counts, resource use, and network state. |
| GET | /api/{api_version}/server/{admin_path}/system/config | Server administrator | Read system settings. | No body; returns current system settings. |
| PUT | /api/{api_version}/server/{admin_path}/system/config | Server administrator | Change system settings. | Settings document; returns the resulting settings or operation result. |
| GET | /api/{api_version}/server/{admin_path}/users | Server administrator | List managed accounts. | No body; returns account identity, role, enabled state, and creation time. |
| POST | /api/{api_version}/server/{admin_path}/users | Server administrator | Create a managed account. | Username, optional email, role, and initial credential; returns account summary. |
| GET | /api/{api_version}/server/{admin_path}/users/{user_id} | Server administrator | Read an account. | User ID; returns account summary. |
| PUT | /api/{api_version}/server/{admin_path}/users/{user_id} | Server administrator | Change account settings or permissions. | User ID and changed account fields; returns updated account summary. |
| DELETE | /api/{api_version}/server/{admin_path}/users/{user_id} | Server administrator | Remove an account. | User ID; returns operation result. |
| GET | /api/{api_version}/server/{admin_path}/nodes | Server administrator | List cluster nodes. | No body; returns node identity, address, role, health, and join time. |
| DELETE | /api/{api_version}/server/{admin_path}/nodes/{node_id} | Server administrator | Remove a cluster node. | Node ID; returns operation result. |
| GET | /api/{api_version}/server/{admin_path}/tokens | Server administrator | List managed API tokens. | No body; returns token owner, scopes, creation, and expiry metadata. |
| POST | /api/{api_version}/server/{admin_path}/tokens | Server administrator | Create an API token. | Name, scopes, optional lifetime; returns token and metadata. |
| DELETE | /api/{api_version}/server/{admin_path}/tokens/{token_id} | Server administrator | Revoke an API token. | Token ID; returns operation result. |
| GET | /api/{api_version}/server/{admin_path}/certificates | Server administrator | List managed certificates. | No body; returns certificate identity, type, expiry, and renewal state. |
| POST | /api/{api_version}/server/{admin_path}/certificates | Server administrator | Request or register a certificate. | Domain, certificate type, renewal preference; returns certificate summary. |
| DELETE | /api/{api_version}/server/{admin_path}/certificates/{cert_id} | Server administrator | Remove a managed certificate. | Certificate ID; returns operation result. |
| GET | /api/{api_version}/server/{admin_path}/backup-jobs | Server administrator | List scheduled backup jobs. | No body; returns job name, schedule, source, destination, and enabled state. |
| POST | /api/{api_version}/server/{admin_path}/backup-jobs | Server administrator | Create a scheduled backup job. | Name, schedule, source type and ID, destination; returns job summary. |
| PUT | /api/{api_version}/server/{admin_path}/backup-jobs/{job_id} | Server administrator | Update a scheduled backup job. | Job ID and changed job fields; returns updated job summary. |
| DELETE | /api/{api_version}/server/{admin_path}/backup-jobs/{job_id} | Server administrator | Delete a scheduled backup job. | Job ID; returns operation result. |
| GET | /api/{api_version}/server/{admin_path}/compliance | Server administrator | Read compliance profile state. | No body; returns profile status and enabled state. |
| PUT | /api/{api_version}/server/{admin_path}/compliance/{type} | Server administrator | Change a compliance profile. | Profile type and settings; returns resulting profile state. |
| GET | /api/{api_version}/server/{admin_path}/audit-logs | Server administrator | Search administrative audit history. | Optional time range, user, action, and result limit; returns matching audit events. |
| GET | /api/{api_version}/server/{admin_path}/security/events | Server administrator | Review security events. | No body; returns security event summaries. |
| GET | /api/{api_version}/vms | Authorized resource user or administrator | List visible virtual machines. | Optional filters; returns VM identity, state, allocation, placement, and creation time. |
| POST | /api/{api_version}/vms | Authorized user or administrator | Create a virtual machine. | Name, memory, CPU count, disk size, and OS type; returns VM summary. |
| GET | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Read a virtual machine. | VM ID; returns VM configuration and status. |
| PUT | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Update permitted VM settings. | VM ID and changed settings; returns updated VM summary. |
| DELETE | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Delete a virtual machine. | VM ID; returns operation result. |
| GET | /api/{api_version}/vms/{vm_id}/status | Authorized resource user or administrator | Read VM lifecycle status. | VM ID; returns current state. |
| GET | /api/{api_version}/vms/{vm_id}/snapshots | Authorized resource user or administrator | List VM snapshots. | VM ID; returns snapshot summaries. |
| POST | /api/{api_version}/vms/{vm_id}/snapshots | Authorized resource user or administrator | Create a VM snapshot. | VM ID and snapshot options; returns snapshot summary. |
| POST | /api/{api_version}/vms/{vm_id}/migrations | Server administrator | Request migration of a VM to another node. | Destination node and migration options; returns migration operation summary. |
| POST | /api/{api_version}/vm-operations | Authorized resource user or administrator | Request a VM lifecycle operation. | VM ID and action (start, stop, or restart); returns operation summary and resulting state. |
| GET | /api/{api_version}/containers | Authorized resource user or administrator | List visible containers. | Optional filters; returns container summaries. |
| POST | /api/{api_version}/containers | Authorized user or administrator | Create a container. | Container configuration; returns container summary. |
| GET | /api/{api_version}/containers/{container_id} | Authorized resource user or administrator | Read a container. | Container ID; returns container configuration and state. |
| PUT | /api/{api_version}/containers/{container_id} | Authorized resource user or administrator | Update permitted container settings. | Container ID and changed settings; returns updated summary. |
| DELETE | /api/{api_version}/containers/{container_id} | Authorized resource user or administrator | Delete a container. | Container ID; returns operation result. |
| POST | /api/{api_version}/container-operations | Authorized resource user or administrator | Request a container lifecycle operation. | Container ID and action (start, stop, or restart); returns operation summary and resulting state. |
| GET | /api/{api_version}/networks | Server administrator | List virtual networks. | No body; returns network summaries. |
| POST | /api/{api_version}/networks | Server administrator | Create a virtual network. | Network configuration; returns network summary. |
| GET | /api/{api_version}/networks/{network_id} | Server administrator | Read a virtual network. | Network ID; returns network configuration and status. |
| PUT | /api/{api_version}/networks/{network_id} | Server administrator | Update a virtual network. | Network ID and changed settings; returns updated network summary. |
| DELETE | /api/{api_version}/networks/{network_id} | Server administrator | Delete a virtual network. | Network ID; returns operation result. |
| GET | /api/{api_version}/storage/pools | Server administrator | List storage pools. | No body; returns pool summaries and capacity. |
| POST | /api/{api_version}/storage/pools | Server administrator | Create a storage pool. | Pool configuration; returns pool summary. |
| GET | /api/{api_version}/storage/pools/{pool_id} | Server administrator | Read a storage pool. | Pool ID; returns pool configuration and capacity. |
| DELETE | /api/{api_version}/storage/pools/{pool_id} | Server administrator | Delete a storage pool. | Pool ID; returns operation result. |
| GET | /api/{api_version}/storage/volumes | Authorized resource user or administrator | List visible storage volumes. | Optional filters; returns volume summaries. |
| POST | /api/{api_version}/storage/volumes | Authorized user or administrator | Create a storage volume. | Volume configuration; returns volume summary. |
| GET | /api/{api_version}/storage/volumes/{volume_id} | Authorized resource user or administrator | Read a storage volume. | Volume ID; returns volume details and status. |
| DELETE | /api/{api_version}/storage/volumes/{volume_id} | Authorized resource user or administrator | Delete an authorized storage volume. | Volume ID; returns operation result. |
| GET | /api/{api_version}/storage/backups | Server administrator | List available backups. | Optional filters; returns backup and restore-point summaries. |
| POST | /api/{api_version}/storage/backups | Server administrator | Create a backup operation. | Source and backup options; returns operation or backup summary. |
| GET | /api/{api_version}/monitoring | Server administrator | Query monitoring data. | Time range and resolution filters; returns time-series results. |
| GET | /api/{api_version}/monitoring/prometheus | Monitoring bearer credential | Machine-only metrics scrape. | No body; returns Prometheus text exposition. |
| GET | /api/{api_version}/monitoring/status | Server administrator | Read current system status. | No body; returns host resource use, network totals, workload counts, and uptime. |
| GET | /api/{api_version}/monitoring/nodes/{node_id}/metrics | Server administrator | Read metrics for a node. | Node ID; returns node metrics. |
| GET | /api/{api_version}/monitoring/alerts | Server administrator | List alerts. | Optional filters; returns alert summaries. |
| POST | /api/{api_version}/monitoring/alerts | Server administrator | Create an alert. | Alert definition; returns created alert summary. |
| GET | /api2/json/version | Compatibility API client | Machine-only: read compatibility API version information. | No body; returns version and release identifiers. |
| GET | /api2/json/nodes | Compatibility API credential | Machine-only: list compatibility API nodes. | No body; returns node summaries and capacity. |
| GET | /api2/json/nodes/{node}/status | Compatibility API credential | Machine-only: read node status. | Node name; returns uptime and CPU, memory, and swap state. |
| GET | /api2/json/nodes/{node}/qemu | Compatibility API credential | Machine-only: list node virtual machines. | Node name; returns QEMU workload summaries. |
| GET | /api2/json/nodes/{node}/lxc | Compatibility API credential | Machine-only: list node containers. | Node name; returns LXC workload summaries. |
| GET | /api2/json/nodes/{node}/storage | Compatibility API credential | Machine-only: list node storage. | Node name; returns storage capacity and availability. |
| GET | /api2/json/cluster/status | Compatibility API credential | Machine-only: read compatibility cluster status. | No body; returns node and cluster status summaries. |
| GET | /api2/json/cluster/resources | Compatibility API credential | Machine-only: list cluster resources. | No body; returns resource identity, capacity, and status. |
| POST | /api2/json/access/ticket | Anonymous credential exchange | Machine-only: create a compatibility authentication ticket. | Username, password, and any protocol-defined challenge response; returns the ticket and CSRF token in the native compatibility response. |
| GET | /api2/json/access/users | Compatibility API credential | Machine-only: list compatibility access users. | No body; returns user identity and enabled/expiry metadata. |
