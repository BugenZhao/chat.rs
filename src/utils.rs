pub fn new_name(name: String) -> String {
    if name.is_empty() {
        names::Generator::default().next().unwrap()
    } else {
        name
    }
}
