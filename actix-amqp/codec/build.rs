#[cfg(feature = "from-spec")]
extern crate handlebars;
#[cfg(feature = "from-spec")]
#[macro_use]
extern crate lazy_static;
#[cfg(feature = "from-spec")]
extern crate serde;
#[cfg(feature = "from-spec")]
#[macro_use]
extern crate serde_derive;
#[cfg(feature = "from-spec")]
extern crate regex;
#[cfg(feature = "from-spec")]
extern crate serde_json;

#[cfg(feature = "from-spec")]
mod codegen;

fn main() {
    #[cfg(feature = "from-spec")]
    {
        generate_from_spec();
    }
}

#[cfg(feature = "from-spec")]
fn generate_from_spec() {
    use handlebars::{Handlebars, Helper, RenderContext, RenderError};
    use std::env;
    use std::fs::File;
    use std::io::Write;

    let template = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/codegen/definitions.rs"
    ));
    let spec = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/codegen/specification.json"
    ));
    let out_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not defined")
        + "/src/protocol/";

    let definitions = codegen::parse(spec);

    let mut codegen = Handlebars::new();
    codegen.register_helper(
        "snake",
        Box::new(
            |h: &Helper, _: &Handlebars, rc: &mut RenderContext| -> Result<(), RenderError> {
                let value = h
                    .param(0)
                    .ok_or_else(|| RenderError::new("Param not found for helper \"snake\""))?;
                let param = value.value().as_str().ok_or_else(|| {
                    RenderError::new("Non-string param given to helper \"snake\"")
                })?;
                rc.writer.write_all(codegen::snake_case(param).as_bytes())?;
                Ok(())
            },
        ),
    );

    codegen
        .register_template_string("definitions", template.to_string())
        .expect("Failed to register template.");
    let mut data = std::collections::BTreeMap::new();

    data.insert("defs", definitions);
    let def_path = std::path::Path::new(&out_dir).join("definitions.rs");
    {
        let mut f = File::create(def_path.clone()).expect("Failed to create target file.");
        let rendered = codegen
            .render("definitions", &data)
            .expect("Failed to render template.");
        writeln!(f, "{}", rendered).expect("Failed to write to file.");
    }

    reformat_file(&def_path);

    fn reformat_file(path: &std::path::Path) {
        use std::fs::OpenOptions;
        use std::io::{Read, Seek};
        std::process::Command::new("rustfmt")
            .arg(path.to_str().unwrap())
            .output()
            .expect("failed to format definitions.rs");

        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("failed to open file");
        let mut data = String::new();
        f.read_to_string(&mut data).expect("failed to read file.");
        let regex = regex::Regex::new("(?P<a>[\r]?\n)[\\s]*\r?\n").unwrap();
        data = regex.replace_all(&data, "$a").into();
        f.seek(std::io::SeekFrom::Start(0)).unwrap();
        f.set_len(data.len() as u64).unwrap();
        f.write_all(data.as_bytes())
            .expect("Error writing reformatted file");
    }
}
