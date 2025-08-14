# AZenith for Developers
Introducing Rom Integrated AZenith for developers/Rom Maintainer,
Developed By @Zexshia and @rianixia on telegram

# How to Add AZenith into my vendor?
1. Place vendor.azenith-service binary to /vendor/bin/hw/
2. Place init.azenith.rc to /vendor/etc/init/
3. Place AZenith_Profiler and AZenith_config to /vendor/bin/
4. Patch vendor sepolicy
5. In /vendor/buid.prop add "persist.sys.azenith.state" prop 1 for enabled 0 for disabled
6. Repack Vendor

# Credits
- @Kombat
- @Kaminarich
