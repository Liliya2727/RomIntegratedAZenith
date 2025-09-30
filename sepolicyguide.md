# AZenith Service SELinux Policy Patch

This document describes how to define SELinux types, contexts, and permissions for the **AZenith service** and its helper scripts. The goal is to integrate the service into the Android SELinux policy while ensuring the correct security boundaries.

---

## 1. Type and Attribute Definitions

We need to define new **types** and **attributes** for:

* The **AZenith service process**
* Its **executable file**
* The **system properties** it manages

These definitions belong in:

```
/vendor/etc/selinux/[vendor_sepolicy.cil](vendor_sepolicy.cil)
```

### Attribute Declarations

* `azenith_service` → added to the `domain` attribute
* `azenith_prop` → added to the `property` attribute

**Example:**

```cil
(typeattribute domain (azenith_service adbd_31 ...))
(typeattribute property_type (azenith_prop system_prop ...))
```

### Type Definitions

At the bottom of the file (with other type declarations), add:

```cil
(type azenith_service)
(roletype object_r azenith_service)

(type azenith_service_exec)
(roletype object_r azenith_service_exec)

(type azenith_prop)
```

### Permissions and Transitions

Allow the `init` process to interact with the property and execute the service:

```cil
(allow init azenith_prop (file (relabelto write read getattr map open)))
(allow azenith_prop tmpfs (filesystem (associate)))
(typetransition init azenith_service_exec process azenith_service)
```

---

## 2. File and Property Contexts

Define the file contexts in:

```
/vendor/etc/selinux/vendor_file_context
```

```text
/vendor/bin/hw/vendor.azenith-service    u:object_r:azenith_service_exec:s0
/vendor/bin/AZenith_Profiler             u:object_r:vendor_shell_exec:s0
/vendor/bin/AZenith_config               u:object_r:vendor_shell_exec:s0
```

Define the property contexts in:

```
/vendor/etc/selinux/vendor_property_context
```

```text
sys.azenith.            u:object_r:azenith_prop:s0
persist.sys.azenith.    u:object_r:azenith_prop:s0
```

---

## 3. Possible Denials (DIY)

### Denials may vary on different device/ROMs so address denials yourself!!
Example denials might include:

* Attempts by `azenith_service` to access `sysfs` or `procfs`
* Property service access restrictions

Keep iterating until all legitimate functionality works without `avc: denied` spam.

# Good luck.