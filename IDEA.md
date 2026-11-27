## Project description

CasVPS is a self-hosted virtualization management platform for homelabs and larger server environments. It gives administrators a web interface and API to manage virtual machines, containers, storage, networking, and the hosts that run them. The project aims to make those capabilities manageable from a small ARM-based server through to a multi-node datacenter, with monitoring, backups, security controls, and recovery features included in the same product.

## Project variables

project_name: casvps
project_org: webappsrust
# STABLE — set at first-time setup; change only on an explicit rename or fork reset
internal_name: casvps
# STABLE — set at first-time setup; change only on an explicit rename or fork reset
internal_org: webappsrust
app_name: CasVPS
resource_label: virtual machine
resource_label_plural: virtual machines

## Business logic

### Product scope & non-goals

CasVPS manages virtualization hosts and the resources running on them. Its intended users are homelab administrators and operators of larger server environments who need one place to provision and operate virtual machines, containers, and their supporting infrastructure.

The product scope includes:

- Create, inspect, configure, start, stop, restart, and remove virtual machines and containers.
- Manage VM snapshots, migration, images and installation media, and guest lifecycle settings.
- Organize storage pools and volumes; schedule, retain, and restore backups.
- Create and manage virtual networks, address allocations, and network services used by virtualized workloads.
- View host, node, VM, container, storage, and network health and metrics; configure alerts and service checks.
- Operate multiple hosts as a cluster, including node membership, resource placement, high availability, and rolling maintenance.
- Manage certificates, security monitoring, firewall policy, and system compliance profiles relevant to virtualization operations.
- Provide a browser-based management experience and an API suitable for automation. Preserve the documented Proxmox VE API compatibility surface for clients that rely on it.
- Provide operational support information, event and audit history, and recovery actions that help administrators diagnose and recover from failures.

CasVPS is a virtualization management product. It does not promise that every optional enterprise integration or compliance profile is enabled in every deployment. It does not provide public cloud hosting, a general-purpose application marketplace, or a managed service operated on behalf of customers.

### Roles & permissions

- **Server administrator:** configures and operates the CasVPS installation, hosts, cluster, security settings, and all managed resources.
- **Resource user:** uses resources assigned to them and may perform permitted lifecycle and configuration actions on those resources. They must not gain access to another user's resources or host-wide administrative controls.
- **Automation client:** acts within the permissions granted to its credential and is intended for API-based administration or resource operations.

Administrative actions and resource access must be attributable to the responsible human or automation identity. The product must keep host-wide control distinct from permissions over an individual user's workloads.

### Data model & sensitivity

The product manages the following information:

- **Hosts and cluster nodes:** identity, availability, capacity, health, and cluster membership.
- **Virtual machines and containers:** names, guest configuration, lifecycle state, assigned resources, ownership, and placement.
- **Storage and backups:** pools, volumes, snapshots, backup jobs, restore points, and retention information.
- **Networks and addresses:** virtual network configuration, subnets, address allocations, and workload associations.
- **Operating system media and templates:** source, version, architecture, availability, and local inventory.
- **Monitoring and operations:** metrics, alerts, service status, scheduled work, events, and audit records.
- **Security and compliance state:** certificates, security findings, policy settings, and compliance results.
- **Identity and access information:** administrator and resource-user identities, permissions, and credential metadata.

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

### Trust boundaries & abuse cases

- Administrators, resource users, automation clients, and unauthenticated network callers have different trust levels. A caller must not gain higher privileges by changing an object identifier, node name, or request payload.
- VM and container guests are untrusted and may be malicious. Guest activity must not grant access to the host, the management plane, other workloads, or cluster credentials.
- Network requests, uploaded media, templates, and imported workload images are untrusted inputs. They may be malformed, oversized, malicious, or crafted to exhaust host resources.
- Cluster peers and remote backup or image sources cross host and network trust boundaries. A compromised or unreachable peer must not silently authorize administrative changes or corrupt unrelated workload state.
- Relevant abuse cases include credential theft, unauthorized workload access, privilege escalation, destructive API requests, denial of service through resource exhaustion, malicious guest escape attempts, tampering with backups or audit history, and exposure of sensitive host or network details.
- The product must make security-relevant administrative and workload actions auditable and must avoid exposing credentials or private key material in user-visible output or operational records.

### API routes

The API inventory below describes product operations available to authorized callers. Resource operations require permission for the target resource; host, cluster, security, and administrative operations require server-administrator permission. Exact request and response fields follow the API contract and implementation specification.

| Method | Path | Access | Purpose |
|--------|------|--------|---------|
| GET | /api/{api_version}/vms | Authorized resource user or administrator | List workloads visible to the caller. |
| POST | /api/{api_version}/vms | Authorized user or administrator | Create a virtual machine. |
| GET | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Read a virtual machine. |
| PUT | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Update permitted VM settings. |
| DELETE | /api/{api_version}/vms/{vm_id} | Authorized resource user or administrator | Delete a virtual machine. |
| POST | /api/{api_version}/vms/{vm_id}/start | Authorized resource user or administrator | Start a virtual machine. |
| POST | /api/{api_version}/vms/{vm_id}/stop | Authorized resource user or administrator | Stop a virtual machine. |
| POST | /api/{api_version}/vms/{vm_id}/restart | Authorized resource user or administrator | Restart a virtual machine. |
| GET | /api/{api_version}/vms/{vm_id}/status | Authorized resource user or administrator | Read VM lifecycle status. |
| GET, POST | /api/{api_version}/vms/{vm_id}/snapshots | Authorized resource user or administrator | List or create VM snapshots. |
| POST | /api/{api_version}/vms/{vm_id}/migrate | Server administrator | Migrate a VM to an eligible node. |
| GET, POST | /api/{api_version}/containers | Authorized resource user or administrator | List or create containers. |
| GET, PUT, DELETE | /api/{api_version}/containers/{container_id} | Authorized resource user or administrator | Read, update, or delete a container. |
| POST | /api/{api_version}/containers/{container_id}/start | Authorized resource user or administrator | Start a container. |
| POST | /api/{api_version}/containers/{container_id}/stop | Authorized resource user or administrator | Stop a container. |
| POST | /api/{api_version}/containers/{container_id}/restart | Authorized resource user or administrator | Restart a container. |
| GET, POST | /api/{api_version}/networks | Server administrator | List or create virtual networks. |
| GET, PUT, DELETE | /api/{api_version}/networks/{network_id} | Server administrator | Read, update, or delete a virtual network. |
| GET, POST | /api/{api_version}/storage/pools | Server administrator | List or create storage pools. |
| GET, DELETE | /api/{api_version}/storage/pools/{pool_id} | Server administrator | Read or remove a storage pool. |
| GET, POST | /api/{api_version}/storage/volumes | Authorized resource user or administrator | List or create volumes within granted storage. |
| GET, DELETE | /api/{api_version}/storage/volumes/{volume_id} | Authorized resource user or administrator | Read or remove an authorized volume. |
| GET, POST | /api/{api_version}/storage/backups | Server administrator | List backups or create a backup operation. |
| GET | /api/{api_version}/monitoring | Server administrator | Query monitoring data. |
| GET | /api/{api_version}/monitoring/status | Server administrator | Read system status. |
| GET | /api/{api_version}/monitoring/nodes/{node_id}/metrics | Server administrator | Read metrics for a cluster node. |
| GET, POST | /api/{api_version}/monitoring/alerts | Server administrator | List or create monitoring alerts. |
| GET | /api2/json/* | Authorized API client | Use the documented Proxmox VE compatibility endpoints for nodes, cluster status, and access information. |
