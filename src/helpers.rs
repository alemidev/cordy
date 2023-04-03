use mlua::{Value, Table};

pub fn pretty_lua(val: Value) -> String {
	// TODO there must be some builtin to do this, right???
	match val {
		Value::Nil => "nil".into(),
		Value::Boolean(b) => if b { "true".into() } else { "false".into() },
		Value::LightUserData(x) => format!("LightUserData({:?})", x),
		Value::Integer(n) => format!("{}", n),
		Value::Number(n) => format!("{:.3}", n),
		Value::String(s) => s.to_str().expect("string is not str").into(),
		Value::Table(t) => try_serialize_table(&t),
		Value::Function(f) => format!("Function({:?}", f),
		Value::Thread(t) => format!("Thread({:?})", t),
		Value::UserData(x) => format!("UserData({:?})", x),
		Value::Error(e) => format!("Error({:?}) : {}", e, e.to_string()),
	}
}

fn try_serialize_table(t: &Table) -> String {
	match serde_json::to_string(t) {
		Ok(txt) => txt,
		Err(_e)  => format!("{:?}", t),
	}
}
