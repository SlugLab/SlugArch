pub fn parse_ptx_raw(text: &str) -> Result<ptx_parser::ast::Module<'_>, Vec<ptx_parser::PtxError<'_>>> {
    ptx_parser::parse_module_checked(text)
}
