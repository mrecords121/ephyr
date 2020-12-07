use actix_web_static_files::NpmBuild;

fn main() -> anyhow::Result<()> {
    NpmBuild::new("./")
        .executable("yarn")
        .install()?
        .run(if cfg!(debug_assertions) {
            "build:dev"
        } else {
            "build:prod"
        })?
        .target("./public")
        .to_resource_dir()
        .build()?;
    Ok(())
}
