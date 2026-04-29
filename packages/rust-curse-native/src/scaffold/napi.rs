use super::{BuildContribution, Scaffold, ScaffoldError, ScaffoldSection};

pub struct NapiSectionOpts {
    pub feature_name: String,
    pub content: String,
}

impl Scaffold {
    pub fn set_napi(&mut self, opts: NapiSectionOpts) -> Result<(), ScaffoldError> {
        let section = ScaffoldSection::new("napi")
            .cargo_feature_flag(&opts.feature_name)
            .lib(opts.content)
            .build(BuildContribution {
                preamble: "use napi_build;".into(),
                main: "napi_build::setup();".into(),
            })
            .dep("napi", "3")
            .dep("napi-derive", "3")
            .build_dep("napi-build", "2");

        self.set(section)
    }
}
