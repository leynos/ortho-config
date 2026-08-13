use ortho_config::OrthoConfig;

#[derive(OrthoConfig)]
struct Colliding {
    foo_bar: String,
    #[ortho_config(cli_long = "unrelated")]
    fooBar: String,
}
