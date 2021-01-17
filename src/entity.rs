use std::string::ToString;


pub struct Entity {
    pub name: String,
    pub is_host: bool,
    pub pos: (f32, f32, f32),
}

impl Entity {
    pub fn new(
        name: String,
        is_host: bool,
        pos: (f32, f32, f32)
    ) -> Self
    {
        Self {name, is_host, pos}
    }
}

impl ToString for Entity {
    fn to_string(&self) -> String {
        String::from("some json")
    }
}
