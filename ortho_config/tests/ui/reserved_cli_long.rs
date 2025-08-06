use ortho_config::OrthoConfig;

#[derive(OrthoConfig)]
struct Bad {
    #[ortho_config(cli_long = "help")]
    field: String,
}
