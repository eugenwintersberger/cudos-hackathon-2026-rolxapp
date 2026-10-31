fn main() {
    #[cfg(feature = "ui")]
    {
        let config = slint_build::CompilerConfiguration::new()
            .with_style("fluent-light".into())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
        slint_build::compile_with_config("ui/login.slint", config).expect("slint compile failed");
    }
}
