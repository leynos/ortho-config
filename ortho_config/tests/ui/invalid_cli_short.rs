use ortho_config::OrthoConfig;

#[derive(OrthoConfig)]
struct Bad {
    #[ortho_config(cli_short = '?')]
    field: String,
}
