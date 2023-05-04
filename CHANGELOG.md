# Changelog
This is OOD Service Changelog, it lists all published beta ood service versions

service version can be found in service startup log
You can use Ctrl+F to search for specific service version

## [Unreleased]

## [1.1.1.82] - 2023-04-15
### Improvements:
Optimize bdt uptime logic when no udp endpoint is available  
Protocol stack optimization guessing mime logic  
Protocol stack optimization and DecApp related caching logic  
Issue #156: AppManager optimizes the uninstall logic of DecApp  
Issue #165, #168: Optimization of AppManager's local repo logic  
Issue #170: Optimization of AppManager's state repair logic at startup  
Issue #196: The -overwrite parameter of ood-installer can control whether to overwrite the original system-config.toml file  
Issue #201: Optimize the hash detection logic of the protocol stack chunk reader  
 
### Fixes:
Issue #157: AppManager may not stop the timeout DecApp installation command correctly under Windows  
Fix the service execution problem of AppManager in docker mode.  
Fixed a status problem when AppManager reports decapp to ood-daemon.  
Fixed some problems related to NDN chunk.  
Bdt fixed multiple panic  
Issue #183, #186, #187: Fixed multiple panics in the protocol stack  
Issue #185: Use cyfs-async-h1 to replace the original async-h1 to avoid panic caused by invalid http header  
Issue #198: AppManager does not handle the timeout of install command correctly in docker mode.  
Fix: AppManager incorrectly set the App status when it received the Install command, resulting in the inability to retry the installation logic after reboot  