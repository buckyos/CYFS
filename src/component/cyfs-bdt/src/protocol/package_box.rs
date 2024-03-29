use super::{
    common::*, 
    package::*
};

//TODO: Option<AesKey> 支持明文包
pub struct PackageBox {
    remote: DeviceId,
    key: MixAesKey, 
    packages: Vec<DynamicPackage>,
}

impl std::fmt::Debug for PackageBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PackageBox:{{remote:{},key:{},packages:", self.remote, self.key)?;
        for package in self.packages() {
            use crate::protocol;
            downcast_handle!(package, |p| {
                let _ = write!(f, "{:?};", p);
            });
        }
        write!(f, "}}")
    }
}

impl PackageBox {
    pub fn from_packages(remote: DeviceId, key: MixAesKey, packages: Vec<DynamicPackage>) -> Self {
        // session package 的数组，不合并
        let mut package_box = Self::encrypt_box(remote, key);
        package_box.append(packages);
        package_box
    }

    pub fn from_package(remote: DeviceId, key: MixAesKey, package: DynamicPackage) -> Self {
        let mut package_box = Self::encrypt_box(remote.clone(), key);
        package_box.packages.push(package);
        package_box
    }

    pub fn encrypt_box(remote: DeviceId, key: MixAesKey) -> Self {
        Self {
            remote,
            key,
            packages: vec![],
        }
    }

    pub fn append(&mut self, packages: Vec<DynamicPackage>) -> &mut Self {
        let mut packages = packages;
        self.packages.append(&mut packages);
        self
    }

    pub fn push<T: 'static + Package + Send + Sync>(&mut self, p: T) -> &mut Self {
        self.packages.push(DynamicPackage::from(p));
        self
    }

    pub fn pop(&mut self) -> Option<DynamicPackage> {
        if self.packages.is_empty() {
            None
        } else {
            Some(self.packages.remove(0))
        }
    }

    pub fn remote(&self) -> &DeviceId {
        &self.remote
    }

    pub fn key(&self) -> &MixAesKey {
        &self.key
    }

    pub fn has_exchange(&self) -> bool {
        self.packages.get(0).unwrap().cmd_code().is_exchange()
    }

    pub fn is_sn(&self) -> bool {
        self.packages_no_exchange()
            .get(0)
            .unwrap()
            .cmd_code()
            .is_sn()
    }

    pub fn is_tunnel(&self) -> bool {
        self.packages_no_exchange()
            .get(0)
            .unwrap()
            .cmd_code()
            .is_tunnel()
    }

    pub fn is_tcp_stream(&self) -> bool {
        self.packages_no_exchange()
            .get(0)
            .unwrap()
            .cmd_code()
            .is_tcp_stream()
    }

    pub fn is_proxy(&self) -> bool {
        self.packages_no_exchange()
            .get(0)
            .unwrap()
            .cmd_code()
            .is_proxy()
    }

    pub fn packages(&self) -> &[DynamicPackage] {
        self.packages.as_ref()
    }

    pub fn mut_packages(&mut self) -> &mut [DynamicPackage] {
        self.packages.as_mut()
    }

    pub fn packages_no_exchange(&self) -> &[DynamicPackage] {
        if self.has_exchange() {
            &self.packages()[1..]
        } else {
            self.packages()
        }
    }

    pub fn mut_packages_no_exchage(&mut self) -> &mut [DynamicPackage] {
        if self.has_exchange() {
            &mut self.mut_packages()[1..]
        } else {
            self.mut_packages()
        }
    }
}

impl Into<Vec<DynamicPackage>> for PackageBox {
    fn into(self) -> Vec<DynamicPackage> {
        self.packages
    }
}
