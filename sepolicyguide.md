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
/vendor/etc/selinux/vendor_sepolicy.cil
```

### Attribute Declarations

* `azenith_service` → added to the `domain` attribute
* `azenith_prop` → added to the `property` attribute

**Example:**

```cil
(typeattributeset domain (azenith_service))
(typeattributeset mlstrustedsubject (azenith_service))
(typeattributeset dev_type (azenith_service))
(typeattributeset extended_core_property_type (azenith_prop))
```

### Type Definitions

At the bottom of the file (with other type declarations), add:

```cil
(type azenith_service)
(roletype object_r azenith_service)
(type azenith_service_exec)
(roletype object_r azenith_service_exec)
(type azenith_prop)
(roletype object_r azenith_prop)
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

`persist.sys.` is already claimed by the platform policy: `plat_property_contexts` maps
`persist.sys.` to `system_prop`, so **do not** add the AZenith prefixes to
`vendor_property_context` — vendor property contexts cannot override a platform-owned prefix,
and the build rejects the overlap.

The service therefore needs access to `system_prop` instead, in `vendor_sepolicy.cil`:

```cil
(allow azenith_service system_prop (file (getattr map open read)))
(allow azenith_service system_prop (property_service (set)))
```

Both prefixes (`persist.sys.azenithIntg.*` for daemon state, `persist.sys.azenithConfIntg.*`
for the tweak toggles) resolve to that one type, so those two rules cover all of them. Apps and
`shell` already read `system_prop` through the platform policy.

If your build trips a `neverallow` on `system_prop` writes, whitelist `azenith_service` the
same way your ROM already handles its other system property writers.

---

## 3. Possible Denials (DIY)

### Denials may vary on different device/ROMs so address denials yourself!!
Example denials might include:

* Attempts by `azenith_service` to access `sysfs` or `procfs`
* Property service access restrictions

Keep iterating until all legitimate functionality works without `avc: denied` spam.


# Good luck.
