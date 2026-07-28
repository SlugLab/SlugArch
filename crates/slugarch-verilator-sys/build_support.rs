pub fn parse_verilator_root(report: &str) -> Option<&str> {
    report.lines().fold(None, |root, line| {
        let Some((key, value)) = line.split_once('=') else {
            return root;
        };
        let value = value.trim();
        if key.trim() == "VERILATOR_ROOT" && !value.is_empty() {
            Some(value)
        } else {
            root
        }
    })
}
