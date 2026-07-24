# Fedora Virtualization Setup Guide (QEMU/KVM + Libvirt)

This document describes the native Fedora QEMU/KVM + libvirt virtualization environment configured on this system.

---

## 1. System & Architecture Overview

* **Operating System**: Fedora Linux 43 (Workstation Edition)
* **Kernel**: Linux 7.1.4-104.fc43.x86_64
* **Hardware**: AMD Ryzen 9 9900X 12-Core Processor
* **Virtualization Tech**: AMD-V hardware virtualization enabled (`/dev/kvm` ready with `crw-rw-rw-` access)
* **Libvirt Architecture**: Modern modular libvirt daemons (`virtqemud.socket`, `virtnetworkd.socket`, etc.) utilizing systemd socket activation. The legacy monolithic `libvirtd.service` is superseded.
* **Target Connection**: `qemu:///system` (system-wide hypervisor instance with direct access to KVM hardware acceleration and system bridge/NAT networking).

---

## 2. Installed & Required Components

| Component | Status | Package / Executable | Purpose |
| :--- | :--- | :--- | :--- |
| **QEMU/KVM** | Installed | `qemu-kvm` (10.1.5) | Hardware hypervisor & emulator |
| **Libvirt Daemon** | Installed | `libvirt-daemon-kvm` (11.6.0) | Core virtualization API & daemon infrastructure |
| **Libvirt Client** | Installed | `libvirt-client` (11.6.0) / `/usr/bin/virsh` | Management CLI tool |
| **Virt Manager** | Installed | `virt-manager` (5.1.0) / `/usr/bin/virt-manager` | Desktop GUI for VM administration |
| **QEMU Disk Tool** | Installed | `qemu-img` / `/usr/bin/qemu-img` | Image manipulation (qcow2 creation, inspection, resize) |
| **Virt Install** | Pending Install | `sudo dnf install virt-install` | CLI automated VM installation script |

---

## 3. Fedora-Specific Service Details

Fedora 43 defaults to **modular libvirt sockets**. Instead of running `libvirtd.service` continuously, systemd listens on sockets and activates sub-daemons on demand.

* **Primary Daemon Socket**: `virtqemud.socket` (Listen target: `/run/libvirt/virtqemud-sock`)
* **Network Daemon Socket**: `virtnetworkd.socket`
* **Storage Daemon Socket**: `virtstoraged.socket`

To verify or restart libvirt sockets:
```bash
systemctl status virtqemud.socket
sudo systemctl restart virtqemud.socket
```

---

## 4. Permission & Group Configuration

* **Libvirt Group**: User `fcosta` is a member of the `libvirt` group (gid `987`).
* **Rootless Management**: Membership in `libvirt` allows running `virsh` commands and launching `virt-manager` directly without `sudo` or running as `root`.

---

## 5. GUI & CLI Health Checks

### GUI Launch
Launch the Virtual Machine Manager GUI from the application menu or terminal:
```bash
virt-manager --connect qemu:///system
```

### CLI Health Checks
```bash
# Verify URI connection
virsh --connect qemu:///system uri

# Inspect host system capabilities
virsh --connect qemu:///system nodeinfo

# Verify KVM device access
ls -l /dev/kvm
```

---

## 6. Network & Storage Pool Inspection

### Default NAT Network
```bash
# List all virtual networks
virsh --connect qemu:///system net-list --all

# View default network details (IP range, DHCP range, bridge name)
virsh --connect qemu:///system net-dumpxml default

# Start or set autostart for default network if needed
virsh --connect qemu:///system net-start default
virsh --connect qemu:///system net-autostart default
```

### Default Storage Pool
```bash
# List storage pools
virsh --connect qemu:///system pool-list --all

# View storage pool capacity and path
virsh --connect qemu:///system pool-info default

# Output storage pool path (Default: /var/lib/libvirt/images)
virsh --connect qemu:///system pool-dumpxml default | grep path
```

---

## 7. Basic VM Lifecycle Workflow

### Creating a VM (CLI)
Once `virt-install` is installed:
```bash
virt-install \
  --connect qemu:///system \
  --name nixos-test \
  --memory 4096 \
  --vcpus 2 \
  --disk size=20,format=qcow2,pool=default \
  --os-variant nixos-unstable \
  --cdrom /path/to/nixos.iso \
  --graphics spice
```

### Listing VMs
```bash
# List running VMs
virsh --connect qemu:///system list

# List all VMs (running & stopped)
virsh --connect qemu:///system list --all
```

### Managing VM Power State
```bash
# Start a VM
virsh --connect qemu:///system start <vm-name>

# Graceful shutdown
virsh --connect qemu:///system shutdown <vm-name>

# Force off (resemble power pull)
virsh --connect qemu:///system destroy <vm-name>

# Reboot
virsh --connect qemu:///system reboot <vm-name>
```

### Deleting a VM
```bash
# Undefine VM definition (retains storage disk unless --remove-all-storage is added)
virsh --connect qemu:///system undefine <vm-name> --remove-all-storage
```

---

## 8. Inspection, XML & Disk Images

### Inspect VM Configuration (XML)
```bash
# Dump active XML configuration
virsh --connect qemu:///system dumpxml <vm-name>

# Edit VM configuration in text editor
virsh --connect qemu:///system edit <vm-name>
```

### Finding Disk Image Locations
```bash
# Extract disk image path from VM XML
virsh --connect qemu:///system dumpxml <vm-name> | grep -E "<source file="

# Inspect disk image details (format, virtual size, actual size)
qemu-img info /var/lib/libvirt/images/<vm-disk-name>.qcow2
```

---

## 9. Snapshot Management

```bash
# Take an internal snapshot (when VM is running or shut down)
virsh --connect qemu:///system snapshot-create-as <vm-name> <snapshot-name> --description "Pre-upgrade snapshot"

# List snapshots for a VM
virsh --connect qemu:///system snapshot-list <vm-name>

# Revert to a snapshot
virsh --connect qemu:///system snapshot-revert <vm-name> <snapshot-name>

# Delete a snapshot
virsh --connect qemu:///system snapshot-delete <vm-name> <snapshot-name>
```

---

## 10. Troubleshooting & FAQ

| Problem | Cause | Resolution |
| :--- | :--- | :--- |
| `Permission denied` on `qemu:///system` | User not in `libvirt` group | Run `sudo usermod -aG libvirt $USER` and log out/in. |
| `/dev/kvm` missing or permission error | Hardware virtualization disabled in BIOS or kernel module not loaded | Check `lscpu | grep Virtualization`, enable AMD-V/VT-x in BIOS, ensure `kvm_amd` / `kvm_intel` module loaded. |
| `Network default is not active` | NAT network was stopped | Run `virsh --connect qemu:///system net-start default` and `virsh --connect qemu:///system net-autostart default`. |
| `virtqemud.socket` not responding | Socket unit inactive | Run `sudo systemctl enable --now virtqemud.socket`. |
