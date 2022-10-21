# Changelog
This is OOD Service Changelog, it lists all published beta ood service versions

service version can be found in service startup log
You can use Ctrl+F to search for specific service version

## [Unreleased]

## [1.0.1.42] - 2022-10-21
### Add
- Add encryption and decryption to the crypto module of the cyfs stack
### Changes
- Increase the minimum rust version to 1.63
- Refactor the logic of dsg-services
- Optimize the uninstall logic of DecApp
- Optimize download logic of Service and DecApp to reduce CPU usage
- bdt performance improve
- Improve the stability of master-slave OOD synchronization
### Fix
- Fix the mount path of data directory in DecApp Service sandbox.