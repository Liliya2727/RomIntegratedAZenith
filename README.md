# AZenith NightlyBuilds for Developers (Tests Builds)
Introducing Rom Integrated AZenith for developers/Rom Maintainer,
Developed By @Zexshia and @rianixia on telegram

# How to Add AZenith into my vendor?
1. Place vendor.azenith-service binary to /vendor/bin/hw/
2. Place init.azenith.rc to /vendor/etc/init/
3. Place AZenith_Profiler and AZenith_config to /vendor/bin/
4. Patch vendor sepolicy
5. In /vendor/build.prop add "persist.sys.azenith.state" prop value 1 for enabled, and value 0 for disabled
6. Repack Vendor

# How add your app packages?
1. Create the file gamelist.txt on /sdcard/gamelist.txt
2. Add your package inside the file for example "com.mobile.legends" if you want more package just add them in new lines
For Example
```
com.mobile.legends
adventure.rpg.anime.game.vng.ys6
age.of.civilizations2.jakowski.lukasz
air.com.ace2three.mobile.cash
(and so on)
```
3. That's it! Enjoy

# Credits
- @Kombat
- @Kaminarich
- @Rem01Gaming
