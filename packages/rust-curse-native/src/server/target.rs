use napi_derive::napi;

/// Supported Rust targets for native Node.js builds.
#[napi]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServerNodeTarget {
    Aarch64AppleDarwin = 1,
    Aarch64LinuxAndroid = 18,
    Aarch64PcWindowsMsvc = 5,
    Aarch64UnknownFreebsd = 21,
    Aarch64UnknownLinuxGnu = 8,
    Aarch64UnknownLinuxMusl = 9,
    Aarch64UnknownLinuxOhos = 22,
    Armv7LinuxAndroideabi = 19,
    Armv7UnknownLinuxGnueabihf = 10,
    Armv7UnknownLinuxMusleabihf = 11,
    Armv7UnknownLinuxOhos = 24,
    I686PcWindowsMsvc = 4,
    Loongarch64UnknownLinuxGnu = 14,
    Loongarch64UnknownLinuxMusl = 15,
    Powerpc64leUnknownLinuxGnu = 16,
    Riscv64gcUnknownLinuxGnu = 12,
    Riscv64gcUnknownLinuxMusl = 13,
    S390xUnknownLinuxGnu = 17,
    X86_64AppleDarwin = 0,
    X86_64PcWindowsGnu = 3,
    X86_64PcWindowsMsvc = 2,
    X86_64UnknownFreebsd = 20,
    X86_64UnknownLinuxGnu = 6,
    X86_64UnknownLinuxMusl = 7,
    X86_64UnknownLinuxOhos = 23,
}

impl ServerNodeTarget {
    pub fn triple(self) -> &'static str {
        match self {
            Self::Aarch64AppleDarwin => "aarch64-apple-darwin",
            Self::Aarch64LinuxAndroid => "aarch64-linux-android",
            Self::Aarch64PcWindowsMsvc => "aarch64-pc-windows-msvc",
            Self::Aarch64UnknownFreebsd => "aarch64-unknown-freebsd",
            Self::Aarch64UnknownLinuxGnu => "aarch64-unknown-linux-gnu",
            Self::Aarch64UnknownLinuxMusl => "aarch64-unknown-linux-musl",
            Self::Aarch64UnknownLinuxOhos => "aarch64-unknown-linux-ohos",
            Self::Armv7LinuxAndroideabi => "armv7-linux-androideabi",
            Self::Armv7UnknownLinuxGnueabihf => "armv7-unknown-linux-gnueabihf",
            Self::Armv7UnknownLinuxMusleabihf => "armv7-unknown-linux-musleabihf",
            Self::Armv7UnknownLinuxOhos => "armv7-unknown-linux-ohos",
            Self::I686PcWindowsMsvc => "i686-pc-windows-msvc",
            Self::Loongarch64UnknownLinuxGnu => "loongarch64-unknown-linux-gnu",
            Self::Loongarch64UnknownLinuxMusl => "loongarch64-unknown-linux-musl",
            Self::Powerpc64leUnknownLinuxGnu => "powerpc64le-unknown-linux-gnu",
            Self::Riscv64gcUnknownLinuxGnu => "riscv64gc-unknown-linux-gnu",
            Self::Riscv64gcUnknownLinuxMusl => "riscv64gc-unknown-linux-musl",
            Self::S390xUnknownLinuxGnu => "s390x-unknown-linux-gnu",
            Self::X86_64AppleDarwin => "x86_64-apple-darwin",
            Self::X86_64PcWindowsGnu => "x86_64-pc-windows-gnu",
            Self::X86_64PcWindowsMsvc => "x86_64-pc-windows-msvc",
            Self::X86_64UnknownFreebsd => "x86_64-unknown-freebsd",
            Self::X86_64UnknownLinuxGnu => "x86_64-unknown-linux-gnu",
            Self::X86_64UnknownLinuxMusl => "x86_64-unknown-linux-musl",
            Self::X86_64UnknownLinuxOhos => "x86_64-unknown-linux-ohos",
        }
    }

    pub fn host() -> Option<Self> {
        [
            Self::Aarch64AppleDarwin,
            Self::Aarch64LinuxAndroid,
            Self::Aarch64PcWindowsMsvc,
            Self::Aarch64UnknownFreebsd,
            Self::Aarch64UnknownLinuxGnu,
            Self::Aarch64UnknownLinuxMusl,
            Self::Aarch64UnknownLinuxOhos,
            Self::Armv7LinuxAndroideabi,
            Self::Armv7UnknownLinuxGnueabihf,
            Self::Armv7UnknownLinuxMusleabihf,
            Self::Armv7UnknownLinuxOhos,
            Self::I686PcWindowsMsvc,
            Self::Loongarch64UnknownLinuxGnu,
            Self::Loongarch64UnknownLinuxMusl,
            Self::Powerpc64leUnknownLinuxGnu,
            Self::Riscv64gcUnknownLinuxGnu,
            Self::Riscv64gcUnknownLinuxMusl,
            Self::S390xUnknownLinuxGnu,
            Self::X86_64AppleDarwin,
            Self::X86_64PcWindowsGnu,
            Self::X86_64PcWindowsMsvc,
            Self::X86_64UnknownFreebsd,
            Self::X86_64UnknownLinuxGnu,
            Self::X86_64UnknownLinuxMusl,
            Self::X86_64UnknownLinuxOhos,
        ]
        .into_iter()
        .find(|target| target.triple() == env!("RUST_CURSE_TARGET"))
    }

    /// The sibling filename expected by `compiler/codegen/ffi/napi-loader.mjs`.
    pub fn native_binary_name(self) -> String {
        let suffix = match self {
            Self::Aarch64AppleDarwin => "darwin-arm64",
            Self::Aarch64LinuxAndroid => "android-arm64",
            Self::Aarch64PcWindowsMsvc => "win32-arm64-msvc",
            Self::Aarch64UnknownFreebsd => "freebsd-arm64",
            Self::Aarch64UnknownLinuxGnu => "linux-arm64-gnu",
            Self::Aarch64UnknownLinuxMusl => "linux-arm64-musl",
            Self::Aarch64UnknownLinuxOhos => "openharmony-arm64",
            Self::Armv7LinuxAndroideabi => "android-arm-eabi",
            Self::Armv7UnknownLinuxGnueabihf => "linux-arm-gnueabihf",
            Self::Armv7UnknownLinuxMusleabihf => "linux-arm-musleabihf",
            Self::Armv7UnknownLinuxOhos => "openharmony-arm",
            Self::I686PcWindowsMsvc => "win32-ia32-msvc",
            Self::Loongarch64UnknownLinuxGnu => "linux-loong64-gnu",
            Self::Loongarch64UnknownLinuxMusl => "linux-loong64-musl",
            Self::Powerpc64leUnknownLinuxGnu => "linux-ppc64-gnu",
            Self::Riscv64gcUnknownLinuxGnu => "linux-riscv64-gnu",
            Self::Riscv64gcUnknownLinuxMusl => "linux-riscv64-musl",
            Self::S390xUnknownLinuxGnu => "linux-s390x-gnu",
            Self::X86_64AppleDarwin => "darwin-x64",
            Self::X86_64PcWindowsGnu => "win32-x64-gnu",
            Self::X86_64PcWindowsMsvc => "win32-x64-msvc",
            Self::X86_64UnknownFreebsd => "freebsd-x64",
            Self::X86_64UnknownLinuxGnu => "linux-x64-gnu",
            Self::X86_64UnknownLinuxMusl => "linux-x64-musl",
            Self::X86_64UnknownLinuxOhos => "openharmony-x64",
        };
        format!("index.{suffix}.node")
    }
}
